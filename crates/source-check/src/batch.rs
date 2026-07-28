//! Batch MCP check — one `start_check_sources` per chunk.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use source_mcp::{batch_check_urls, batch_max_wait_s, FsChannelPort, McpClient, McpEndpoint, repo_root};
use source_types::PortError;

#[derive(Debug, Clone)]
pub struct BatchCheckOpts {
    pub urls: Vec<String>,
    pub keyword: String,
    pub thread_count: u32,
    pub batch_size: usize,
    pub timeout_ms: u64,
    pub check_discovery: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BatchCheckSummary {
    pub started: usize,
    pub success: usize,
    pub failed: usize,
    pub results: Vec<Value>,
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

    for chunk in opts.urls.chunks(opts.batch_size.max(1)) {
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
        for row in rows {
            summary.started += 1;
            if row
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                summary.success += 1;
            } else {
                summary.failed += 1;
            }
            summary.results.push(row);
        }
        thread::sleep(Duration::from_millis(400));
    }
    Ok(summary)
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
