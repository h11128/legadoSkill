//! Host EWMA file IO — parity with `repair_cache` note_* / cooldown_for.

use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::disk::{CachePaths, IoResult};
use crate::ewma::{
    apply_rate_limit, apply_verify_success, cooldown_seconds, HostStat, DEFAULT_GAP_S,
};
use crate::keys::host_of;

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn load_hosts(paths: &CachePaths) -> IoResult<BTreeMap<String, HostStat>> {
    paths.ensure()?;
    if !paths.host_stats.is_file() {
        return Ok(BTreeMap::new());
    }
    let data: Value = match serde_json::from_str(&fs::read_to_string(&paths.host_stats)?) {
        Ok(v) => v,
        Err(_) => return Ok(BTreeMap::new()),
    };
    let mut out = BTreeMap::new();
    if let Some(obj) = data.as_object() {
        for (k, v) in obj {
            if let Ok(row) = serde_json::from_value::<HostStat>(v.clone()) {
                out.insert(k.clone(), row);
            }
        }
    }
    Ok(out)
}

pub fn save_hosts(paths: &CachePaths, data: &BTreeMap<String, HostStat>) -> IoResult<()> {
    paths.ensure()?;
    fs::write(&paths.host_stats, serde_json::to_string_pretty(data)?)?;
    Ok(())
}

/// Python `note_rate_limit`.
pub fn note_rate_limit(paths: &CachePaths, url: &str, suggested_gap_s: f64) -> IoResult<HostStat> {
    let host = host_of(url);
    let mut data = load_hosts(paths)?;
    let mut row = data.remove(&host).unwrap_or_default();
    apply_rate_limit(&mut row, suggested_gap_s, now());
    data.insert(host, row.clone());
    save_hosts(paths, &data)?;
    Ok(row)
}

/// Python `note_verify`.
pub fn note_verify(
    paths: &CachePaths,
    url: &str,
    success: bool,
    duration_ms: u64,
    used_cooldown_s: f64,
) -> IoResult<HostStat> {
    let host = host_of(url);
    let mut data = load_hosts(paths)?;
    let mut row = data.remove(&host).unwrap_or_else(|| HostStat {
        ewma_gap_s: DEFAULT_GAP_S,
        ..HostStat::default()
    });
    apply_verify_success(&mut row, success, duration_ms, used_cooldown_s, now());
    data.insert(host, row.clone());
    save_hosts(paths, &data)?;
    Ok(row)
}

/// Python `cooldown_for`.
pub fn cooldown_for(paths: &CachePaths, url: &str, concurrent_rate: Option<&str>) -> IoResult<f64> {
    let host = host_of(url);
    let data = load_hosts(paths)?;
    let gap = data
        .get(&host)
        .map(|r| r.ewma_gap_s)
        .unwrap_or(DEFAULT_GAP_S);
    Ok(cooldown_seconds(gap, concurrent_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::CachePaths;
    use tempfile::TempDir;

    #[test]
    fn note_and_cooldown_persist() {
        let dir = TempDir::new().unwrap();
        let paths = CachePaths::from_cache_dir(dir.path());
        let url = "https://a.example/";
        note_rate_limit(&paths, url, 20.0).unwrap();
        let cd = cooldown_for(&paths, url, None).unwrap();
        assert!((cd - 8.1).abs() < 1e-6);
        note_verify(&paths, url, true, 100, 3.0).unwrap();
        assert!(paths.host_stats.is_file());
    }
}
