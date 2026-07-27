//! Cluster verify-ok samples into PatternCluster (§10.5).

use std::collections::HashMap;

use serde_json::{json, Value};
use source_types::{
    Fingerprint, PartialBookSource, PatternCluster, RepairConfig, SiteFamily, Url,
};

use crate::fields::{chapter_list, content_rule, search_book_list, search_url, source_type};
use crate::hash::{normalize_search_url_shape, structural_hash_from_source};

/// One verify-ok (or candidate) source row for clustering.
#[derive(Debug, Clone)]
pub struct ClusterSample {
    pub url: Url,
    pub source: source_types::BookSource,
    pub verify_ok: bool,
}

/// Group exact `structural_hash`, require `cluster_min_size`, emit provisional family ids.
pub fn cluster_verify_ok(
    samples: &[ClusterSample],
    config: &RepairConfig,
    extracted_at: &str,
) -> Vec<PatternCluster> {
    let mut groups: HashMap<String, Vec<&ClusterSample>> = HashMap::new();
    for s in samples.iter().filter(|s| s.verify_ok) {
        let h = structural_hash_from_source(&s.source);
        groups.entry(h).or_default().push(s);
    }

    let min = config.cluster_min_size as usize;
    let mut out = Vec::new();
    for (hash, members) in groups {
        if members.len() < min {
            continue;
        }
        let family = SiteFamily::new(format!("cluster_{}", &hash[..8.min(hash.len())]));
        let signals = collect_signals(&members);
        let confidence = (members.len() as f64 / samples.len().max(1) as f64).clamp(0.0, 1.0);
        let fingerprint = Fingerprint {
            signals,
            structural_hash: hash,
            confidence,
        };
        let (centroid, coverage) = centroid_and_coverage(&members);
        let mut exemplars: Vec<Url> = members.iter().map(|m| m.url.clone()).collect();
        exemplars.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        exemplars.truncate(5);
        let size = members.len() as u32;
        let mut cluster = PatternCluster::new(
            family,
            size,
            fingerprint,
            centroid,
            exemplars,
            extracted_at,
        );
        cluster.coverage = coverage;
        out.push(cluster);
    }
    out.sort_by(|a, b| a.fingerprint.structural_hash.cmp(&b.fingerprint.structural_hash));
    out
}

fn collect_signals(members: &[&ClusterSample]) -> Vec<String> {
    let mut set: HashMap<String, ()> = HashMap::new();
    for m in members {
        if let Some(su) = search_url(&m.source) {
            let shape = normalize_search_url_shape(&su);
            set.insert(format!("shape:{shape}"), ());
        }
        if let Some(bl) = search_book_list(&m.source) {
            set.insert(format!("list:{bl}"), ());
        }
    }
    let mut v: Vec<String> = set.into_keys().collect();
    v.sort();
    v
}

fn mode_string(values: &[String]) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in values {
        *counts.entry(v.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(s, _)| s.to_string())
}

fn field_values(members: &[&ClusterSample], getter: fn(&source_types::BookSource) -> Option<String>) -> Vec<String> {
    members.iter().filter_map(|m| getter(&m.source)).collect()
}

fn centroid_and_coverage(
    members: &[&ClusterSample],
) -> (PartialBookSource, HashMap<String, f64>) {
    let n = members.len() as f64;
    let mut coverage = HashMap::new();
    let mut obj = serde_json::Map::new();

    let pairs: [(&str, fn(&source_types::BookSource) -> Option<String>); 4] = [
        ("searchUrl", search_url),
        ("ruleSearch.bookList", search_book_list),
        ("ruleToc.chapterList", chapter_list),
        ("ruleContent.content", content_rule),
    ];

    for (path, getter) in pairs {
        let vals = field_values(members, getter);
        coverage.insert(path.to_string(), vals.len() as f64 / n);
        if let Some(mode) = mode_string(&vals) {
            set_dotted(&mut obj, path, json!(mode));
        }
    }

    // Prefer modal bookSourceType among members.
    let types: Vec<String> = members.iter().map(|m| source_type(&m.source)).collect();
    if let Some(t) = mode_string(&types) {
        if let Ok(n) = t.parse::<i64>() {
            obj.insert("bookSourceType".into(), json!(n));
        } else {
            obj.insert("bookSourceType".into(), json!(t));
        }
    }

    (PartialBookSource::new(Value::Object(obj)), coverage)
}

fn set_dotted(obj: &mut serde_json::Map<String, Value>, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() == 1 {
        obj.insert(parts[0].into(), value);
        return;
    }
    let head = parts[0];
    let rest = parts[1..].join(".");
    let entry = obj.entry(head.to_string()).or_insert_with(|| json!({}));
    if let Value::Object(map) = entry {
        set_dotted(map, &rest, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::BookSource;

    fn sample(url: &str, book_list: &str) -> ClusterSample {
        ClusterSample {
            url: Url::new(url).unwrap(),
            source: BookSource::new(json!({
                "bookSourceUrl": url,
                "searchUrl": "https://host/search.php?q={{key}}",
                "ruleSearch": { "bookList": book_list },
                "ruleToc": { "chapterList": ".ch" },
                "ruleContent": { "content": "#c" },
                "bookSourceType": 0
            })),
            verify_ok: true,
        }
    }

    #[test]
    fn clusters_when_min_size_met() {
        let samples = vec![
            sample("https://a.example/", ".item"),
            sample("https://b.example/", ".item"),
            sample("https://c.example/", ".item"),
        ];
        let cfg = RepairConfig::default();
        let clusters = cluster_verify_ok(&samples, &cfg, "2026-07-27T00:00:00Z");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].size, 3);
        assert!(clusters[0].family.is_provisional_cluster());
        assert!(clusters[0].coverage.get("searchUrl").copied().unwrap_or(0.0) >= 1.0);
    }

    #[test]
    fn skips_small_groups() {
        let samples = vec![sample("https://a.example/", ".item")];
        let clusters = cluster_verify_ok(&samples, &RepairConfig::default(), "t");
        assert!(clusters.is_empty());
    }
}
