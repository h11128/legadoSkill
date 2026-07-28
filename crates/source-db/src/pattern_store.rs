//! Persist PatternCluster rows + list snapshot payloads for clustering.

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use source_types::PatternCluster;

use crate::Result;

pub fn upsert_cluster(conn: &Connection, cluster: &PatternCluster) -> Result<()> {
    let centroid = serde_json::to_string(cluster.centroid.as_value())?;
    let exemplars = serde_json::to_string(
        &cluster
            .exemplars
            .iter()
            .map(|u| u.as_str().to_string())
            .collect::<Vec<_>>(),
    )?;
    let coverage = serde_json::to_string(&cluster.coverage)?;
    conn.execute(
        "INSERT INTO pattern_cluster(
           family, size, structural_hash, confidence,
           centroid_json, exemplars_json, coverage_json, extracted_at, promoted
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0)
         ON CONFLICT(family) DO UPDATE SET
           size=excluded.size,
           structural_hash=excluded.structural_hash,
           confidence=excluded.confidence,
           centroid_json=excluded.centroid_json,
           exemplars_json=excluded.exemplars_json,
           coverage_json=excluded.coverage_json,
           extracted_at=excluded.extracted_at",
        params![
            cluster.family.as_str(),
            cluster.size as i64,
            cluster.fingerprint.structural_hash,
            cluster.fingerprint.confidence,
            centroid,
            exemplars,
            coverage,
            cluster.extracted_at,
        ],
    )?;
    Ok(())
}

pub fn count_clusters(conn: &Connection) -> Result<i64> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM pattern_cluster", [], |r| r.get(0))?;
    Ok(n)
}

/// All snapshot payloads (optionally enabled-only).
pub fn list_payloads(conn: &Connection, enabled_only: bool) -> Result<Vec<(String, Value)>> {
    let sql = if enabled_only {
        "SELECT source_key, payload_json FROM source_snapshot WHERE enabled=1"
    } else {
        "SELECT source_key, payload_json FROM source_snapshot"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (key, raw) = row?;
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            out.push((key, v));
        }
    }
    Ok(out)
}

/// Latest ledger source_keys that look verify-ok / fixed.
pub fn fixed_source_keys(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT source_key FROM ledger_events
         WHERE report_status='fixed'
            OR result LIKE 'fixed%'
            OR result LIKE '%校验成功%'
         ORDER BY source_key",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_payload(conn: &Connection, source_key: &str) -> Result<Option<Value>> {
    conn.query_row(
        "SELECT payload_json FROM source_snapshot WHERE source_key=?1",
        [source_key],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|raw| serde_json::from_str(&raw).map_err(Into::into))
    .transpose()
}
