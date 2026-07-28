//! Batch MCP check — one `start_check_sources` per chunk.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{json, Map, Value};
use source_mcp::{
    batch_check_urls, batch_max_wait_s, repo_root, FsChannelPort, McpClient, McpEndpoint,
};
use source_types::PortError;

use crate::materials::{classify_results, dump_fail_materials, tag_counts};

#[derive(Debug, Clone)]
pub struct BatchCheckOpts {
    pub urls: Vec<String>,
    pub keyword: String,
    pub thread_count: u32,
    pub batch_size: usize,
    pub timeout_ms: u64,
    pub check_discovery: bool,
    pub materials_dir: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct BatchCheckSummary {
    pub started: usize,
    pub success: usize,
    pub failed: usize,
    pub results: Vec<Value>,
    pub by_failure_tag: Map<String, Value>,
}

pub fn run_batch_check(opts: BatchCheckOpts) -> Result<BatchCheckSummary, PortError> {
    let root = repo_root()?;
    let channel = FsChannelPort::new(&root);
    let _bulk_guard = channel.acquire_bulk()?;

    let ep = McpEndpoint::load_defaults()?;
    let client = McpClient::new(ep).with_client_name("source_check_batch");
    client.ensure_session()?;

    let mut summary = BatchCheckSummary::default();
    let per_url_timeout_s = opts.timeout_ms as f64 / 1000.0;
    let mut batches: Vec<Value> = Vec::new();

    for (bi, chunk) in opts.urls.chunks(opts.batch_size.max(1)).enumerate() {
        let max_wait = batch_max_wait_s(chunk.len(), per_url_timeout_s);
        let rows = batch_check_urls(
            &client,
            chunk,
            &opts.keyword,
            opts.thread_count,
            opts.timeout_ms,
            max_wait,
            opts.check_discovery,
        )?;
        for row in &rows {
            summary.started += 1;
            if row.get("success").and_then(|v| v.as_bool()) == Some(true) {
                summary.success += 1;
            } else {
                summary.failed += 1;
            }
            summary.results.push(row.clone());
        }
        batches.push(json!({
            "batch": bi,
            "n": chunk.len(),
            "success": rows.iter().filter(|r| r.get("success").and_then(|v| v.as_bool()) == Some(true)).count(),
        }));
        thread::sleep(Duration::from_millis(400));
    }

    let classified = classify_results(&summary.results);
    summary.by_failure_tag = tag_counts(&classified);

    if let Some(dir) = &opts.materials_dir {
        dump_fail_materials(&classified, dir)?;
    }

    let report = json!({
        "started": summary.started,
        "success": summary.success,
        "failed": summary.failed,
        "by_failure_tag": summary.by_failure_tag,
        "batches": batches,
        "results": summary.results,
        "materials_dir": opts.materials_dir.as_ref().map(|p| p.display().to_string()),
    });

    if let Some(path) = &opts.report_path {
        write_json(path, &report)?;
    }

    Ok(summary)
}

fn write_json(path: &Path, value: &Value) -> Result<(), PortError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PortError::Permanent(format!("mkdir: {e}")))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| PortError::Permanent(e.to_string()))?,
    )
    .map_err(|e| PortError::Permanent(format!("write {}: {e}", path.display())))?;
    Ok(())
}

pub fn load_urls_file(path: &Path) -> Result<Vec<String>, PortError> {
    let raw = fs::read_to_string(path).map_err(|e| PortError::Permanent(e.to_string()))?;
    Ok(raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

pub fn load_alive_from_precheck(path: &Path) -> Result<Vec<String>, PortError> {
    let raw = fs::read_to_string(path).map_err(|e| PortError::Permanent(e.to_string()))?;
    let doc: Value = serde_json::from_str(&raw).map_err(|e| PortError::Permanent(e.to_string()))?;
    if let Some(arr) = doc.get("alive_urls").and_then(|v| v.as_array()) {
        return Ok(arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect());
    }
    Ok(doc
        .get("results")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter(|r| r.get("dns_ok").and_then(|v| v.as_bool()) == Some(true))
        .filter_map(|r| r.get("url").and_then(|v| v.as_str()).map(String::from))
        .collect())
}

pub fn dedupe_urls(urls: Vec<String>) -> Vec<String> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for u in urls {
        if seen.insert(u.clone(), ()).is_none() {
            out.push(u);
        }
    }
    out
}
