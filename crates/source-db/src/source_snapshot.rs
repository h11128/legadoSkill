//! BookSource snapshots pulled from device MCP (avoid repeated get_source / list_sources).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSnapshotRow {
    pub source_key: String,
    pub host_key: String,
    pub name: Option<String>,
    pub book_source_type: i64,
    pub enabled: bool,
    pub group_name: Option<String>,
    pub respond_time_ms: Option<i64>,
    pub payload_json: String,
    pub pulled_at: String,
}

pub fn upsert(conn: &Connection, row: &SourceSnapshotRow) -> Result<()> {
    conn.execute(
        "INSERT INTO source_snapshot(
           source_key, host_key, name, type, enabled, group_name,
           respond_time_ms, payload_json, pulled_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(source_key) DO UPDATE SET
           host_key=excluded.host_key,
           name=excluded.name,
           type=excluded.type,
           enabled=excluded.enabled,
           group_name=excluded.group_name,
           respond_time_ms=excluded.respond_time_ms,
           payload_json=excluded.payload_json,
           pulled_at=excluded.pulled_at",
        params![
            row.source_key,
            row.host_key,
            row.name,
            row.book_source_type,
            i64::from(row.enabled),
            row.group_name,
            row.respond_time_ms,
            row.payload_json,
            row.pulled_at,
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, source_key: &str) -> Result<Option<SourceSnapshotRow>> {
    conn.query_row(
        "SELECT source_key, host_key, name, type, enabled, group_name,
                respond_time_ms, payload_json, pulled_at
         FROM source_snapshot WHERE source_key=?1",
        [source_key],
        |r| {
            Ok(SourceSnapshotRow {
                source_key: r.get(0)?,
                host_key: r.get(1)?,
                name: r.get(2)?,
                book_source_type: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
                group_name: r.get(5)?,
                respond_time_ms: r.get(6)?,
                payload_json: r.get(7)?,
                pulled_at: r.get(8)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn count(conn: &Connection) -> Result<i64> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM source_snapshot", [], |r| r.get(0))?;
    Ok(n)
}
