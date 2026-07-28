//! Multi-URL MCP check — one `start_check_sources` per chunk.

use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use source_types::PortError;

use crate::client::McpClient;
use crate::verify::{is_repair_success, match_result, wait_check};

pub fn batch_check_urls(
    client: &McpClient,
    urls: &[String],
    keyword: &str,
    thread_count: u32,
    timeout_ms: u64,
    max_wait_s: f64,
    check_discovery: bool,
) -> Result<Vec<Value>, PortError> {
    client.ensure_session()?;
    let _ = client.tools_call("stop_check_sources", json!({}))?;
    thread::sleep(Duration::from_millis(200));

    let args = json!({
        "urls": urls,
        "enabledOnly": false,
        "keyword": keyword,
        "threadCount": thread_count.max(1),
        "timeoutMs": timeout_ms,
        "checkDomain": false,
        "checkSearch": true,
        "checkDiscovery": check_discovery,
        "checkInfo": true,
        "checkCategory": true,
        "checkContent": true,
    });
    client.tools_call("start_check_sources", args)?;

    let snap = wait_check(client, max_wait_s)?;
    let mut out = Vec::with_capacity(urls.len());
    for url in urls {
        let item = match_result(&snap, url);
        let row = match item {
            Some(row) => {
                let msg = row
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let device_ok = row
                    .get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                let success = if device_ok {
                    true
                } else if !check_discovery {
                    is_repair_success(&msg)
                } else {
                    false
                };
                json!({
                    "url": url,
                    "success": success,
                    "message": msg,
                    "raw": row,
                })
            }
            None => json!({
                "url": url,
                "success": false,
                "message": format!("no check result for {url}"),
            }),
        };
        out.push(row);
    }
    Ok(out)
}

/// Default max wait scales with batch size (mirrors repair_wait heuristic).
pub fn batch_max_wait_s(url_count: usize, per_url_timeout_s: f64) -> f64 {
    let n = url_count.max(1) as f64;
    let scaled = per_url_timeout_s * (1.0 + n * 0.15);
    scaled.min(600.0).max(per_url_timeout_s * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_wait_scales_with_count() {
        assert!(batch_max_wait_s(80, 45.0) > batch_max_wait_s(1, 45.0));
    }
}
