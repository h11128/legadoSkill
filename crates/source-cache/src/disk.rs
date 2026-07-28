//! Disk HTML + triage cache — parity with `repair_cache.get_html` / `put_html` / triage.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use thiserror::Error;

use crate::keys::url_key;

#[derive(Debug, Error)]
pub enum CacheIoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type IoResult<T> = Result<T, CacheIoError>;

#[derive(Debug, Clone)]
pub struct CachePaths {
    pub root: PathBuf,
    pub html_dir: PathBuf,
    pub triage_dir: PathBuf,
    pub host_stats: PathBuf,
}

impl CachePaths {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let cache = root.as_ref().join("temp/full_fix/cache");
        Self {
            root: root.as_ref().to_path_buf(),
            html_dir: cache.join("html"),
            triage_dir: cache.join("triage"),
            host_stats: cache.join("host_stats.json"),
        }
    }

    pub fn from_cache_dir(cache: impl AsRef<Path>) -> Self {
        let cache = cache.as_ref();
        Self {
            root: cache
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or(cache)
                .to_path_buf(),
            html_dir: cache.join("html"),
            triage_dir: cache.join("triage"),
            host_stats: cache.join("host_stats.json"),
        }
    }

    pub fn ensure(&self) -> IoResult<()> {
        fs::create_dir_all(&self.html_dir)?;
        fs::create_dir_all(&self.triage_dir)?;
        if let Some(p) = self.host_stats.parent() {
            fs::create_dir_all(p)?;
        }
        Ok(())
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Read cached HTML if within `max_age_s`. Returns meta + `body` bytes + `cache_hit`.
pub fn get_html(paths: &CachePaths, url: &str, max_age_s: f64) -> IoResult<Option<Value>> {
    paths.ensure()?;
    let key = url_key(url);
    let meta_path = paths.html_dir.join(format!("{key}.json"));
    let bin_path = paths.html_dir.join(format!("{key}.bin"));
    if !meta_path.is_file() || !bin_path.is_file() {
        return Ok(None);
    }
    let meta: Value = serde_json::from_str(&fs::read_to_string(&meta_path)?)?;
    let saved = meta.get("saved_at").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if now() - saved > max_age_s {
        return Ok(None);
    }
    let body = fs::read(&bin_path)?;
    let mut out = meta;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("body".into(), json!(body));
        obj.insert("cache_hit".into(), json!(true));
    }
    Ok(Some(out))
}

/// Write bytes body + selected meta fields (Python `put_html`).
pub fn put_html_bytes(
    paths: &CachePaths,
    url: &str,
    body: &[u8],
    result: &Value,
) -> IoResult<String> {
    paths.ensure()?;
    let key = url_key(url);
    let mut meta = serde_json::Map::new();
    for k in [
        "ok",
        "status",
        "final_url",
        "content_type",
        "bytes",
        "rate_limited",
        "toc_candidate_links",
        "snippet",
    ] {
        if let Some(v) = result.get(k) {
            meta.insert(k.to_string(), v.clone());
        }
    }
    meta.insert("url".into(), json!(url));
    meta.insert("saved_at".into(), json!(now()));
    if !meta.contains_key("bytes") {
        meta.insert("bytes".into(), json!(body.len()));
    }
    let meta_v = Value::Object(meta);
    fs::write(
        paths.html_dir.join(format!("{key}.json")),
        serde_json::to_string_pretty(&meta_v)?,
    )?;
    fs::write(paths.html_dir.join(format!("{key}.bin")), body)?;
    Ok(key)
}

pub fn put_triage(paths: &CachePaths, url: &str, report: &Value) -> IoResult<()> {
    paths.ensure()?;
    let key = url_key(url);
    let path = paths.triage_dir.join(format!("{key}.json"));
    fs::write(path, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

pub fn get_triage(paths: &CachePaths, url: &str, max_age_s: f64) -> IoResult<Option<Value>> {
    let key = url_key(url);
    let path = paths.triage_dir.join(format!("{key}.json"));
    if !path.is_file() {
        return Ok(None);
    }
    let data: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    let cached = data
        .get("cached_at")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    if now() - cached > max_age_s {
        return Ok(None);
    }
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn html_roundtrip_ttl() {
        let dir = TempDir::new().unwrap();
        let paths = CachePaths::from_cache_dir(dir.path());
        let url = "https://a.example/x";
        let key =
            put_html_bytes(&paths, url, b"<html/>", &json!({"ok": true, "status": 200})).unwrap();
        assert_eq!(key.len(), 24);
        let hit = get_html(&paths, url, 3600.0).unwrap().unwrap();
        assert_eq!(hit["cache_hit"], true);
        assert!(get_html(&paths, url, 0.0).unwrap().is_none());
    }

    #[test]
    fn triage_roundtrip() {
        let dir = TempDir::new().unwrap();
        let paths = CachePaths::from_cache_dir(dir.path());
        let url = "https://a.example/y";
        put_triage(&paths, url, &json!({"cached_at": now(), "layer": "toc"})).unwrap();
        assert_eq!(
            get_triage(&paths, url, 1800.0).unwrap().unwrap()["layer"],
            "toc"
        );
    }
}
