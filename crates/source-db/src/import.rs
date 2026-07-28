//! Import JSONL ledger + host_stats.json into SQLite (`repair_db` / `repair_db_cache_meta`).

use std::collections::HashSet;
use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;

use crate::host_stats::{upsert as upsert_host, HostStatsRow};
use crate::keys::{iso_now, norm_source_key};
use crate::Result;

/// Import session ledger JSONL (dedupe by ts|url|step|result within file).
pub fn import_jsonl_ledger(conn: &Connection, path: &Path) -> Result<usize> {
    if !path.is_file() {
        return Ok(0);
    }
    let text = std::fs::read_to_string(path)?;
    let mut seen = HashSet::new();
    let mut n = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let url = norm_source_key(row.get("url").and_then(|v| v.as_str()).unwrap_or(""));
        if url.is_empty() {
            continue;
        }
        let ts = row
            .get("ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let step = row
            .get("step")
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default();
        let result = row
            .get("result")
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default();
        let dedupe = format!("{ts}|{url}|{step}|{result}");
        if !seen.insert(dedupe) {
            continue;
        }
        let ts_final = if ts.is_empty() { iso_now() } else { ts };
        let note = row
            .get("note")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let waste = row
            .get("waste")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        conn.execute(
            "INSERT INTO ledger_events(
               ts, source_key, step, result, note, waste, row_json
             ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![ts_final, url, step, result, note, waste, line],
        )?;
        n += 1;
    }
    Ok(n)
}

pub fn ledger_event_count(conn: &Connection) -> Result<i64> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM ledger_events", [], |r| r.get(0))?;
    Ok(n)
}

fn host_row_from_json(host: &str, row: &Value) -> HostStatsRow {
    HostStatsRow {
        host_key: host.to_string(),
        ewma_gap_s: row
            .get("ewma_gap_s")
            .and_then(|v| v.as_f64())
            .unwrap_or(3.0),
        hits: row.get("hits").and_then(|v| v.as_i64()).unwrap_or(0),
        ok: row.get("ok").and_then(|v| v.as_i64()).unwrap_or(0),
        fail: row.get("fail").and_then(|v| v.as_i64()).unwrap_or(0),
        rate_limits: row.get("rate_limits").and_then(|v| v.as_i64()).unwrap_or(0),
        last_rate_limit_at: row.get("last_rate_limit_at").and_then(|v| v.as_f64()),
        last_duration_ms: row.get("last_duration_ms").and_then(|v| v.as_i64()),
        last_at: row.get("last_at").and_then(|v| v.as_f64()),
        extra_json: None,
    }
}

/// Import `host_stats.json` map into SQLite.
pub fn import_host_stats_file(conn: &Connection, path: &Path) -> Result<usize> {
    if !path.is_file() {
        return Ok(0);
    }
    let data: Value = match serde_json::from_str(&std::fs::read_to_string(path)?) {
        Ok(v) => v,
        Err(_) => return Ok(0),
    };
    let Some(obj) = data.as_object() else {
        return Ok(0);
    };
    let mut n = 0usize;
    for (host, row) in obj {
        if let Some(map) = row.as_object() {
            // re-wrap as Value for helper
            let v = Value::Object(map.clone());
            upsert_host(conn, &host_row_from_json(host, &v))?;
            n += 1;
        }
    }
    Ok(n)
}
