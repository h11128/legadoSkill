//! MCP LAN discovery — probe config + optional adb fallback.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
use source_types::PortError;

use crate::endpoint::McpEndpoint;

const DEFAULT_PORT: u16 = 1236;
const MCP_PATH: &str = "/mcp";

pub fn mcp_url_for(host: &str, port: u16) -> String {
    let host = host.trim().trim_matches(['[', ']']);
    format!("http://{host}:{port}{MCP_PATH}")
}

pub fn probe_mcp(url: &str, token: &str, timeout_s: f64) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs_f64(timeout_s.max(1.0)))
        .build();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "source_mcp_discover", "version": "1.0"},
        }
    });
    let resp = agent
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json, text/event-stream")
        .set("X-Legado-Token", token)
        .send_string(&body.to_string());
    resp.map(|r| r.status() >= 200 && r.status() < 500)
        .unwrap_or(false)
}

fn adb_phone_ip() -> Option<String> {
    let out = Command::new("adb")
        .args(["shell", "ip", "route"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.contains("wlan") {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                return Some(parts[8].to_string());
            }
        }
    }
    None
}

pub fn discover(timeout_s: f64) -> Result<Value, PortError> {
    let defaults = McpEndpoint::load_defaults().ok();
    let token = defaults
        .as_ref()
        .map(|d| d.token.clone())
        .unwrap_or_else(|| "1234".into());

    let mut candidates: Vec<String> = Vec::new();
    if let Some(ep) = &defaults {
        if let Ok(u) = url::Url::parse(&ep.mcp_url) {
            if let Some(h) = u.host_str() {
                candidates.push(h.to_string());
            }
        }
    }
    if let Some(ip) = adb_phone_ip() {
        candidates.push(ip);
    }
    candidates.sort();
    candidates.dedup();

    for host in &candidates {
        let url = mcp_url_for(host, DEFAULT_PORT);
        if probe_mcp(&url, &token, timeout_s) {
            return Ok(json!({
                "found": true,
                "mcp_url": url,
                "web_api": format!("http://{host}:1122"),
                "token": token,
                "via": "probe",
            }));
        }
    }
    Ok(json!({ "found": false, "candidates": candidates }))
}

pub fn write_defaults(discovery: &Value, path: &Path) -> Result<(), PortError> {
    if discovery.get("found") != Some(&json!(true)) {
        return Err(PortError::Permanent("no MCP endpoint discovered".into()));
    }
    let mcp_url = discovery
        .get("mcp_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PortError::Permanent("missing mcp_url".into()))?;
    let token = discovery
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("1234");
    let web_api = discovery
        .get("web_api")
        .and_then(|v| v.as_str())
        .unwrap_or("http://127.0.0.1:1122");
    let payload = json!({
        "mcp_url": mcp_url,
        "token": token,
        "web_api": web_api,
        "updated": chrono::Utc::now().format("%Y-%m-%d").to_string(),
        "note": "Single SOT for phone LAN endpoint. Updated by source-cli discover.",
        "discovered_via": discovery.get("via").cloned().unwrap_or(json!("probe")),
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PortError::Permanent(e.to_string()))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&payload).map_err(|e| PortError::Permanent(e.to_string()))?
            + "\n",
    )
    .map_err(|e| PortError::Permanent(e.to_string()))?;
    Ok(())
}
