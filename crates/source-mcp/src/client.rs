//! Streamable-HTTP MCP JSON-RPC client (matches `scripts/mcp_client.py`).

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use source_types::PortError;

use crate::endpoint::McpEndpoint;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Minimal MCP HTTP client with session header + SSE `data:` unwrap.
pub struct McpClient {
    endpoint: McpEndpoint,
    session: Mutex<Option<String>>,
    timeout: Duration,
    client_name: String,
}

impl McpClient {
    pub fn new(endpoint: McpEndpoint) -> Self {
        Self {
            endpoint,
            session: Mutex::new(None),
            timeout: Duration::from_secs(120),
            client_name: "source_mcp".into(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = name.into();
        self
    }

    pub fn endpoint(&self) -> &McpEndpoint {
        &self.endpoint
    }

    pub fn reset_session(&self) {
        if let Ok(mut g) = self.session.lock() {
            *g = None;
        }
    }

    /// `initialize` + best-effort `notifications/initialized`.
    pub fn ensure_session(&self) -> Result<(), PortError> {
        self.reset_session();
        self.call(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": self.client_name, "version": "1.0"},
            }),
        )?;
        let _ = self.call("notifications/initialized", json!({}));
        Ok(())
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, PortError> {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| (d.as_millis() % 1_000_000_000) as u64)
            .unwrap_or(1);
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let body = serde_json::to_string(&payload).map_err(|e| {
            PortError::ContractViolation(format!("serialize rpc: {e}"))
        })?;

        let mut req = ureq::post(&self.endpoint.mcp_url)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .set("X-Legado-Token", &self.endpoint.token)
            .timeout(self.timeout);

        if let Ok(g) = self.session.lock() {
            if let Some(sid) = g.as_ref() {
                req = req.set("Mcp-Session-Id", sid);
            }
        }

        let resp = req.send_string(&body).map_err(map_transport)?;

        if let Some(sid) = resp.header("Mcp-Session-Id") {
            if let Ok(mut g) = self.session.lock() {
                *g = Some(sid.to_string());
            }
        }

        let body = resp.into_string().map_err(|e| {
            PortError::Transient(format!("read body: {e}"))
        })?;
        let body = unwrap_sse(&body);
        serde_json::from_str(&body).map_err(|e| {
            PortError::ContractViolation(format!("mcp json: {e}; body={}", trunc(&body, 200)))
        })
    }

    pub fn tools_call(&self, name: &str, arguments: Value) -> Result<Value, PortError> {
        let result = self.call(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )?;
        if let Some(err) = result.get("error") {
            return Err(PortError::Permanent(format!("mcp error: {err}")));
        }
        Ok(result.get("result").cloned().unwrap_or(result))
    }

    /// Like Python `extract_text`.
    pub fn extract_text(result: &Value) -> String {
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            if let Some(first) = content.first() {
                if let Some(t) = first.get("text").and_then(|t| t.as_str()) {
                    return t.to_string();
                }
            }
        }
        if let Some(msg) = result.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        result.to_string()
    }

    /// Like Python `parse_json_text`.
    pub fn parse_json_text(text: &str) -> Value {
        let text = text.trim();
        if text.starts_with('{') || text.starts_with('[') {
            serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
        } else {
            json!({ "raw": text })
        }
    }
}

fn unwrap_sse(body: &str) -> String {
    let trimmed = body.trim_start();
    if !(trimmed.starts_with("event:") || body.contains("data:")) {
        return body.to_string();
    }
    let chunks: Vec<&str> = body
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .collect();
    chunks
        .last()
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| body.to_string())
}

fn map_transport(err: ureq::Error) -> PortError {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            PortError::Transient(format!("http {code}: {}", trunc(&body, 200)))
        }
        other => PortError::Transient(format!("mcp transport: {other}")),
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_sse_takes_last_data() {
        let body = "event: message\ndata: {\"a\":1}\ndata: {\"b\":2}\n";
        assert_eq!(unwrap_sse(body), "{\"b\":2}");
    }

    #[test]
    fn extract_text_from_content() {
        let v = json!({"content":[{"type":"text","text":"{\"ok\":true}"}]});
        assert_eq!(McpClient::extract_text(&v), "{\"ok\":true}");
    }

    #[test]
    fn parse_json_object() {
        let v = McpClient::parse_json_text(" {\"x\":1} ");
        assert_eq!(v["x"], 1);
    }
}
