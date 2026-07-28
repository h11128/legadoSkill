//! Phone index helpers — parity with `repair_db_phone.py`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::keys::{host_key, iso_now, norm_source_key, parse_iso_ts};
use crate::source_snapshot::{self, SourceSnapshotRow};
use crate::Result;

/// Upsert MCP list_sources items; stamp `phone_pull_at` / `phone_pull_total`.
pub fn bulk_upsert_list_items(conn: &Connection, items: &[Value]) -> Result<usize> {
    let mut n = 0usize;
    let pulled = iso_now();
    for it in items {
        if let Some(row) = snapshot_from_list_item(it, &pulled) {
            source_snapshot::upsert(conn, &row)?;
            n += 1;
        }
    }
    conn.execute(
        "INSERT INTO schema_meta(key,value) VALUES('phone_pull_at', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&pulled],
    )?;
    conn.execute(
        "INSERT INTO schema_meta(key,value) VALUES('phone_pull_total', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&n.to_string()],
    )?;
    Ok(n)
}

fn snapshot_from_list_item(source: &Value, pulled_at: &str) -> Option<SourceSnapshotRow> {
    let key = norm_source_key(
        source
            .get("bookSourceUrl")
            .or_else(|| source.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );
    if key.is_empty() {
        return None;
    }
    let respond_ms = source
        .get("respondTime")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
    let enabled = source
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let payload = serde_json::to_string(source).ok()?;
    Some(SourceSnapshotRow {
        source_key: key.clone(),
        host_key: host_key(&key),
        name: source
            .get("bookSourceName")
            .or_else(|| source.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        book_source_type: source
            .get("bookSourceType")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        enabled,
        group_name: source
            .get("bookSourceGroup")
            .or_else(|| source.get("group"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        respond_time_ms: respond_ms,
        payload_json: payload,
        pulled_at: pulled_at.to_string(),
    })
}

fn meta_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT value FROM schema_meta WHERE key=?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .map_err(Into::into)
}

/// True when `phone_pull_at` within TTL and snapshots exist.
pub fn phone_index_fresh(conn: &Connection, ttl_s: f64) -> Result<bool> {
    let Some(pulled_s) = meta_value(conn, "phone_pull_at")? else {
        return Ok(false);
    };
    let Some(pulled) = parse_iso_ts(&pulled_s) else {
        return Ok(false);
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if now - pulled > ttl_s {
        return Ok(false);
    }
    Ok(source_snapshot::count(conn)? > 0)
}

/// Export phone_source_index.json shape from DB.
pub fn export_phone_index_json(conn: &Connection, out: &Path) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT source_key, name, group_name, enabled, respond_time_ms, pulled_at
         FROM source_snapshot
         ORDER BY respond_time_ms IS NULL, respond_time_ms ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut by_url = serde_json::Map::new();
    let mut urls = Vec::new();
    for row in rows {
        let (u, name, group, enabled, rt) = row?;
        urls.push(u.clone());
        by_url.insert(
            u.clone(),
            json!({
                "url": u,
                "name": name.unwrap_or_default(),
                "group": group.unwrap_or_default(),
                "enabled": enabled != 0,
                "respondTime": rt,
            }),
        );
    }
    let ts = meta_value(conn, "phone_pull_at")?.unwrap_or_else(iso_now);
    let payload = json!({
        "ts": ts,
        "total": urls.len(),
        "urls": urls,
        "by_url": by_url,
        "from_db": true,
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, serde_json::to_string(&payload)?)?;
    Ok(payload)
}

/// Status blob for CLI `status` (parity with `repair_db_cli`).
pub fn status_json(conn: &Connection, db: &Path, phone_ttl_s: f64) -> Result<Value> {
    let snap = source_snapshot::count(conn)?;
    let led: i64 = conn.query_row("SELECT COUNT(*) FROM ledger_events", [], |r| r.get(0))?;
    let html: i64 = conn.query_row("SELECT COUNT(*) FROM html_cache_meta", [], |r| r.get(0))?;
    let phone_pull_at = meta_value(conn, "phone_pull_at")?;
    Ok(json!({
        "db": db.display().to_string(),
        "source_snapshots": snap,
        "ledger_events": led,
        "html_cache_meta": html,
        "phone_pull_at": phone_pull_at,
        "phone_index_fresh": phone_index_fresh(conn, phone_ttl_s)?,
    }))
}
