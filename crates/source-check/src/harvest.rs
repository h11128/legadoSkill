//! Harvest cheap wins: batch-verify tagged fails with discovery off.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde_json::{json, Value};
use source_mcp::{batch_check_urls, is_repair_success, DualLedgerPort, McpClient, McpEndpoint};
use source_ports::LedgerPort;
use source_types::{LedgerRow, LedgerStep, PortError, Url};

#[derive(Debug, Clone)]
pub struct HarvestOpts {
    pub fails: PathBuf,
    pub limit: usize,
    pub keyword: String,
    pub thread_count: u32,
    pub timeout_ms: u64,
    pub out: PathBuf,
}

pub fn run_harvest(opts: HarvestOpts) -> Result<Value, PortError> {
    let fixed = ledger_fixed_urls()?;
    let skipped = ledger_skipped_urls()?;
    let tried = ledger_harvest_tried()?;
    let urls = load_fail_urls(&opts.fails, opts.limit, &fixed, &skipped, &tried)?;
    if urls.is_empty() {
        return Err(PortError::Permanent("no urls".into()));
    }

    let ep = McpEndpoint::load_defaults()?;
    let client = McpClient::new(ep).with_client_name("source_harvest");
    client.ensure_session()?;
    client.reset_session();
    client.ensure_session()?;

    let started = Instant::now();
    let batches = (urls.len() + opts.thread_count as usize - 1) / opts.thread_count.max(1) as usize;
    let max_wait = (batches as f64 * (opts.timeout_ms as f64 / 1000.0) + 20.0).min(240.0);
    let results = batch_check_urls(
        &client,
        &urls,
        &opts.keyword,
        opts.thread_count,
        opts.timeout_ms,
        max_wait,
        false,
    )?;

    let ledger = DualLedgerPort::from_defaults()?;
    let mut won = Vec::new();
    let mut lost = Vec::new();
    let ts = Utc::now().to_rfc3339();
    for url in &urls {
        let row = results
            .iter()
            .find(|r| r.get("url").and_then(|v| v.as_str()) == Some(url.as_str()));
        let ok = row
            .and_then(|r| {
                if r.get("success").and_then(|v| v.as_bool()) == Some(true) {
                    return Some(true);
                }
                r.get("message")
                    .and_then(|m| m.as_str())
                    .map(is_repair_success)
            })
            .unwrap_or(false);
        let msg = row
            .and_then(|r| r.get("message").and_then(|m| m.as_str()))
            .unwrap_or(if ok { "校验成功" } else { "no result" });
        let u = Url::new(url).map_err(|e| PortError::Permanent(e.to_string()))?;
        let mut lr = LedgerRow::new(
            ts.clone(),
            u,
            LedgerStep::Check,
            if ok {
                source_types::LEDGER_VERIFY_OK.into()
            } else {
                msg.to_string()
            },
        );
        lr.note = Some("harvest".into());
        if !ok {
            lr.waste = Some("needs_deep".into());
        }
        let _ = ledger.append(&lr);
        if ok {
            won.push(url.clone());
        } else {
            lost.push(url.clone());
        }
    }

    let report = json!({
        "wall_s": started.elapsed().as_secs_f64(),
        "n": urls.len(),
        "won": won,
        "lost_n": lost.len(),
        "fixed_n_after": ledger_fixed_urls()?.len(),
        "lost_sample": lost.iter().take(8).collect::<Vec<_>>(),
    });
    if let Some(parent) = opts.out.parent() {
        fs::create_dir_all(parent).map_err(|e| PortError::Permanent(format!("mkdir: {e}")))?;
    }
    fs::write(
        &opts.out,
        serde_json::to_string_pretty(&report).map_err(|e| PortError::Permanent(e.to_string()))?,
    )
    .map_err(|e| PortError::Permanent(format!("write: {e}")))?;
    Ok(report)
}

fn load_fail_urls(
    path: &Path,
    limit: usize,
    fixed: &std::collections::HashSet<String>,
    skipped: &std::collections::HashSet<String>,
    tried: &std::collections::HashSet<String>,
) -> Result<Vec<String>, PortError> {
    let raw = fs::read_to_string(path)
        .map_err(|e| PortError::Permanent(format!("read {}: {e}", path.display())))?;
    let data: Value =
        serde_json::from_str(&raw).map_err(|e| PortError::Permanent(format!("json: {e}")))?;
    let rows = if data.is_array() {
        data.as_array().cloned().unwrap_or_default()
    } else {
        data.get("items")
            .or_else(|| data.get("fails"))
            .or_else(|| data.get("rows"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let Some(obj) = row.as_object() else {
            continue;
        };
        let url = obj
            .get("url")
            .or_else(|| obj.get("bookSourceUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty()
            || fixed.contains(&url)
            || skipped.contains(&url)
            || tried.contains(&url)
            || seen.contains(&url)
        {
            continue;
        }
        if !url.contains("://") && !url.starts_with('/') {
            continue;
        }
        seen.insert(url.clone());
        out.push(url);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

fn norm_url(u: &str) -> String {
    u.trim().trim_end_matches('/').to_string()
}

fn read_ledger_lines() -> Result<Vec<String>, PortError> {
    let path = source_mcp::default_jsonl_path()?;
    Ok(fs::read_to_string(path)
        .map_err(|e| PortError::Permanent(e.to_string()))?
        .lines()
        .map(String::from)
        .collect())
}

fn ledger_fixed_urls() -> Result<std::collections::HashSet<String>, PortError> {
    let mut out = std::collections::HashSet::new();
    for line in read_ledger_lines()? {
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let url = norm_url(row.get("url").and_then(|v| v.as_str()).unwrap_or(""));
        let step = row.get("step").and_then(|v| v.as_str()).unwrap_or("");
        let result = row.get("result").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            continue;
        }
        if step == "check"
            && (result.contains(source_types::LEDGER_VERIFY_OK) || result.starts_with("fixed:"))
        {
            out.insert(url);
        }
    }
    Ok(out)
}

fn ledger_skipped_urls() -> Result<std::collections::HashSet<String>, PortError> {
    let mut out = std::collections::HashSet::new();
    for line in read_ledger_lines()? {
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let url = norm_url(row.get("url").and_then(|v| v.as_str()).unwrap_or(""));
        let step = row.get("step").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            continue;
        }
        if step == "skip" {
            out.insert(url);
        }
    }
    Ok(out)
}

fn ledger_harvest_tried() -> Result<std::collections::HashSet<String>, PortError> {
    let mut out = std::collections::HashSet::new();
    for line in read_ledger_lines()? {
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if row.get("note").and_then(|v| v.as_str()) != Some("harvest") {
            continue;
        }
        let url = norm_url(row.get("url").and_then(|v| v.as_str()).unwrap_or(""));
        if !url.is_empty() {
            out.insert(url);
        }
    }
    Ok(out)
}

pub fn default_fails_path() -> PathBuf {
    for p in [
        PathBuf::from("legado/temp_tagged_fails.json"),
        PathBuf::from("../legado/temp_tagged_fails.json"),
    ] {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("legado/temp_tagged_fails.json")
}
