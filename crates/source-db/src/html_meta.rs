//! `html_cache_meta` upsert + disk import (`repair_db_cache_meta`).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::keys::host_key;
use crate::Result;

#[derive(Debug, Clone)]
pub struct HtmlMetaRow {
    pub cache_key: String,
    pub url: String,
    pub saved_at: f64,
    pub status: Option<i64>,
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub bytes: Option<i64>,
    pub rate_limited: bool,
    pub bin_path: String,
}

fn now_epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn upsert_html_meta(conn: &Connection, row: &HtmlMetaRow) -> Result<()> {
    conn.execute(
        "INSERT INTO html_cache_meta(
           cache_key, url, host_key, saved_at, status, final_url,
           content_type, bytes, rate_limited, bin_path
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(cache_key) DO UPDATE SET
           url=excluded.url, host_key=excluded.host_key,
           saved_at=excluded.saved_at, status=excluded.status,
           final_url=excluded.final_url, content_type=excluded.content_type,
           bytes=excluded.bytes, rate_limited=excluded.rate_limited,
           bin_path=excluded.bin_path",
        params![
            row.cache_key,
            row.url,
            host_key(&row.url),
            row.saved_at,
            row.status,
            row.final_url,
            row.content_type,
            row.bytes,
            i64::from(row.rate_limited),
            row.bin_path,
        ],
    )?;
    Ok(())
}

/// Build row from disk meta JSON (Python `put_html` / import shape).
pub fn row_from_meta_json(
    cache_key: &str,
    url: &str,
    meta: &Value,
    bin_rel: &str,
    bin_bytes: Option<i64>,
) -> HtmlMetaRow {
    let saved_at = meta
        .get("saved_at")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(now_epoch);
    let bytes = meta.get("bytes").and_then(|v| v.as_i64()).or(bin_bytes);
    HtmlMetaRow {
        cache_key: cache_key.to_string(),
        url: url.to_string(),
        saved_at,
        status: meta.get("status").and_then(|v| v.as_i64()),
        final_url: meta
            .get("final_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        content_type: meta
            .get("content_type")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        bytes,
        rate_limited: meta
            .get("rate_limited")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        bin_path: bin_rel.to_string(),
    }
}

/// Scan `html_dir/*.json` + matching `.bin` into `html_cache_meta`.
pub fn import_html_cache_dir(conn: &Connection, html_dir: &Path) -> Result<usize> {
    if !html_dir.is_dir() {
        return Ok(0);
    }
    let mut n = 0usize;
    let mut metas: Vec<std::path::PathBuf> = std::fs::read_dir(html_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    metas.sort();
    for meta_path in metas {
        let key = meta_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if key.is_empty() {
            continue;
        }
        let bin_path = html_dir.join(format!("{key}.bin"));
        if !bin_path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&meta_path)?;
        let meta: Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let url = meta
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            continue;
        }
        let bin_len = std::fs::metadata(&bin_path).ok().map(|m| m.len() as i64);
        let row = row_from_meta_json(&key, &url, &meta, &format!("html/{key}.bin"), bin_len);
        upsert_html_meta(conn, &row)?;
        n += 1;
    }
    Ok(n)
}

pub fn html_meta_count(conn: &Connection) -> Result<i64> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM html_cache_meta", [], |r| r.get(0))?;
    Ok(n)
}
