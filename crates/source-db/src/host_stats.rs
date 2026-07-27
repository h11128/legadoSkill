use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::Result;

/// Row for `host_stats` (§9.3 / EWMA pacing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStatsRow {
    pub host_key: String,
    pub ewma_gap_s: f64,
    pub hits: i64,
    pub ok: i64,
    pub fail: i64,
    pub rate_limits: i64,
    pub last_rate_limit_at: Option<f64>,
    pub last_duration_ms: Option<i64>,
    pub last_at: Option<f64>,
    pub extra_json: Option<String>,
}

impl Default for HostStatsRow {
    fn default() -> Self {
        Self {
            host_key: String::new(),
            ewma_gap_s: 3.0,
            hits: 0,
            ok: 0,
            fail: 0,
            rate_limits: 0,
            last_rate_limit_at: None,
            last_duration_ms: None,
            last_at: None,
            extra_json: None,
        }
    }
}

pub fn upsert(conn: &Connection, row: &HostStatsRow) -> Result<()> {
    conn.execute(
        "INSERT INTO host_stats(
           host_key, ewma_gap_s, hits, ok, fail, rate_limits,
           last_rate_limit_at, last_duration_ms, last_at, extra_json
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(host_key) DO UPDATE SET
           ewma_gap_s=excluded.ewma_gap_s,
           hits=excluded.hits,
           ok=excluded.ok,
           fail=excluded.fail,
           rate_limits=excluded.rate_limits,
           last_rate_limit_at=excluded.last_rate_limit_at,
           last_duration_ms=excluded.last_duration_ms,
           last_at=excluded.last_at,
           extra_json=excluded.extra_json",
        rusqlite::params![
            row.host_key,
            row.ewma_gap_s,
            row.hits,
            row.ok,
            row.fail,
            row.rate_limits,
            row.last_rate_limit_at,
            row.last_duration_ms,
            row.last_at,
            row.extra_json,
        ],
    )?;
    Ok(())
}
