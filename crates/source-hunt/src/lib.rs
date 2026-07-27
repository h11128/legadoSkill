//! Domain hunt from curated seeds (Python `repair_domain_hunt` + seeds JSON).

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum HuntError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SeedEntry {
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub migrated_to: Option<String>,
    #[serde(default)]
    pub shutdown: bool,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HuntSeeds {
    #[serde(default)]
    pub seeds: HashMap<String, SeedEntry>,
}

impl HuntSeeds {
    pub fn load_path(path: &Path) -> Result<Self, HuntError> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Resolve path: explicit arg → `DOMAIN_HUNT_SEEDS` env → `config/domain_hunt_seeds.json`.
    pub fn resolve_path(explicit: Option<&Path>) -> PathBuf {
        if let Some(p) = explicit {
            return p.to_path_buf();
        }
        if let Ok(env) = std::env::var("DOMAIN_HUNT_SEEDS") {
            return PathBuf::from(env);
        }
        PathBuf::from("config/domain_hunt_seeds.json")
    }

    /// Lookup by hostname (with/without www).
    pub fn lookup_host(&self, host: &str) -> Option<&SeedEntry> {
        let h = host.trim_end_matches('.').to_ascii_lowercase();
        if let Some(e) = self.seeds.get(&h) {
            return Some(e);
        }
        if let Some(rest) = h.strip_prefix("www.") {
            if let Some(e) = self.seeds.get(rest) {
                return Some(e);
            }
        }
        self.seeds.get(&format!("www.{h}"))
    }
}

fn hostname(raw: &str) -> Option<String> {
    let base = raw.split('#').next().unwrap_or(raw);
    let base = base.split("##").next().unwrap_or(base).trim();
    let with = if base.contains("://") {
        base.to_string()
    } else {
        format!("http://{base}")
    };
    Url::parse(&with)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

fn entry_candidates(entry: &SeedEntry) -> Vec<String> {
    if entry.shutdown {
        return Vec::new();
    }
    let mut v = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let push = |c: &str, v: &mut Vec<String>, seen: &mut std::collections::HashSet<String>| {
        if seen.insert(c.to_string()) {
            v.push(c.to_string());
        }
    };
    if let Some(to) = &entry.migrated_to {
        push(to, &mut v, &mut seen);
    }
    for c in &entry.candidates {
        push(c, &mut v, &mut seen);
    }
    v
}

/// Candidate URLs for a dead/migrating bookSourceUrl (no network).
pub fn hunt_candidates(seeds: &HuntSeeds, book_source_url: &str) -> Vec<String> {
    let host = hostname(book_source_url).unwrap_or_default();
    if host.is_empty() {
        return Vec::new();
    }
    seeds
        .lookup_host(&host)
        .map(entry_candidates)
        .unwrap_or_default()
}

/// Filter seed entries whose host/note/`migrated_to` contain `keyword` (case-insensitive).
pub fn hunt_candidates_by_keyword(seeds: &HuntSeeds, keyword: &str) -> Vec<String> {
    let kw = keyword.to_ascii_lowercase();
    if kw.is_empty() {
        return Vec::new();
    }
    let mut ordered = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (k, entry) in &seeds.seeds {
        let note = entry.note.as_deref().unwrap_or("");
        let mig = entry.migrated_to.as_deref().unwrap_or("");
        let hay = format!("{k} {note} {mig}").to_ascii_lowercase();
        if !hay.contains(&kw) {
            continue;
        }
        for c in entry_candidates(entry) {
            if seen.insert(c.clone()) {
                ordered.push(c);
            }
        }
    }
    ordered
}

/// Load seeds and return candidates for `dead_url` (optional keyword name filter).
pub fn seed_candidates(
    seeds_path: Option<&Path>,
    dead_url: &str,
    keyword: Option<&str>,
) -> Result<Vec<String>, HuntError> {
    let path = HuntSeeds::resolve_path(seeds_path);
    let seeds = HuntSeeds::load_path(&path)?;
    Ok(match keyword {
        Some(kw) if !kw.is_empty() => hunt_candidates_by_keyword(&seeds, kw),
        _ => hunt_candidates(&seeds, dead_url),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn zxcs_seed_lookup() {
        let seeds: HuntSeeds = serde_json::from_str(
            r#"{"seeds":{"www.zxcs.info":{"migrated_to":"https://www.zxcs.click/","candidates":["https://www.zxcs.click/","https://www.zxcs.live/"]}}}"#,
        )
        .unwrap();
        let c = hunt_candidates(&seeds, "http://www.zxcs.info/");
        assert_eq!(c[0], "https://www.zxcs.click/");
        assert!(c.len() >= 2);
    }

    #[test]
    fn temp_seeds_json_and_keyword() {
        let dir = std::env::temp_dir().join(format!(
            "source_hunt_seeds_{}.json",
            std::process::id()
        ));
        {
            let mut f = std::fs::File::create(&dir).unwrap();
            write!(
                f,
                r#"{{"seeds":{{
                  "www.zxcs.info":{{"note":"zxcs family","migrated_to":"https://www.zxcs.click/","candidates":["https://www.zxcs.live/"]}},
                  "book.tiexue.net":{{"note":"shutdown","shutdown":true,"candidates":["https://book.tiexue.net/"]}}
                }}}}"#
            )
            .unwrap();
        }
        let got = seed_candidates(Some(&dir), "http://www.zxcs.info/", None).unwrap();
        assert!(got.iter().any(|u| u.contains("zxcs.click")));
        let by_kw = seed_candidates(Some(&dir), "http://ignored/", Some("zxcs")).unwrap();
        assert!(by_kw.iter().any(|u| u.contains("zxcs")));
        assert!(!by_kw.iter().any(|u| u.contains("tiexue")));
        let _ = std::fs::remove_file(&dir);
    }
}
