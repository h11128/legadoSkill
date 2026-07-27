use rusqlite::Connection;
use source_types::VerifyResult;

use crate::Result;

/// ISO-8601-ish UTC timestamp for verify_runs.ts (minimal viable).
fn now_ts() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

pub fn record(conn: &Connection, result: &VerifyResult) -> Result<i64> {
    let result_json = serde_json::to_string(result)?;
    let success: i64 = if result.success { 1 } else { 0 };
    let check_discovery: i64 = if result.check_discovery { 1 } else { 0 };
    let duration_ms = result.duration_ms.map(|v| v as i64);
    conn.execute(
        "INSERT INTO verify_runs(
           ts, source_key, success, message, mode, check_discovery,
           duration_ms, capability, result_json
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            now_ts(),
            result.url.as_str(),
            success,
            result.message,
            result.mode.as_str(),
            check_discovery,
            duration_ms,
            Option::<String>::None,
            result_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}
