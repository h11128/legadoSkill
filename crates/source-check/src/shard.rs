//! Consistent-hash URL sharding — parity with `shard_urls.py` (stable FNV-1a).
//!
//! Python used salted `hash()`; Rust uses FNV-1a32 for cross-run stability, then
//! the same Murmur-style `mix` finalizer.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShardError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

pub type ShardResult<T> = Result<T, ShardError>;

/// MurmurHash3 finalizer (matches Python `mix`).
pub fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x & 0x7FFF_FFFF
}

/// Deterministic 32-bit string hash (FNV-1a).
pub fn str_hash32(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

pub fn build_ring(nodes: &[String], virtual_nodes: u32) -> Vec<(u32, String)> {
    let mut ring = Vec::new();
    for node in nodes {
        for v in 0..virtual_nodes {
            let key = mix(str_hash32(&format!("{node}#{v}")));
            ring.push((key, node.clone()));
        }
    }
    ring.sort_by_key(|(k, _)| *k);
    ring
}

pub fn node_for(ring: &[(u32, String)], url: &str) -> Option<String> {
    if ring.is_empty() {
        return None;
    }
    let h = mix(str_hash32(url));
    for (key, node) in ring {
        if *key >= h {
            return Some(node.clone());
        }
    }
    Some(ring[0].1.clone())
}

pub fn load_urls_file(path: &Path) -> ShardResult<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|ln| !ln.is_empty() && !ln.starts_with('#'))
        .map(str::to_string)
        .collect())
}

/// Shard URLs across nodes; returns map node -> urls (BTreeMap for stable JSON).
pub fn shard_urls(
    urls: &[String],
    nodes: &[String],
    virtual_nodes: u32,
) -> ShardResult<BTreeMap<String, Vec<String>>> {
    if nodes.is_empty() {
        return Err(ShardError::Msg("nodes empty".into()));
    }
    let ring = build_ring(nodes, virtual_nodes);
    let mut shards: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in nodes {
        shards.entry(n.clone()).or_default();
    }
    for url in urls {
        if let Some(node) = node_for(&ring, url) {
            shards.entry(node).or_default().push(url.clone());
        }
    }
    Ok(shards)
}

pub fn write_shards(out: &Path, shards: &BTreeMap<String, Vec<String>>) -> ShardResult<Value> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let doc = json!(shards);
    std::fs::write(out, serde_json::to_string_pretty(&doc)?)?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_assigns_deterministically() {
        let nodes = vec!["phoneA".into(), "phoneB".into()];
        let ring = build_ring(&nodes, 64);
        assert_eq!(ring.len(), 128);
        let u = "https://example.com/book/1";
        let a = node_for(&ring, u).unwrap();
        let b = node_for(&ring, u).unwrap();
        assert_eq!(a, b);
        let shards = shard_urls(&[u.into(), "https://other/".into()], &nodes, 64).unwrap();
        assert_eq!(shards.values().map(|v| v.len()).sum::<usize>(), 2);
    }

    #[test]
    fn mix_smoke() {
        assert_eq!(mix(0), 0);
        assert_ne!(mix(1), 1);
    }
}
