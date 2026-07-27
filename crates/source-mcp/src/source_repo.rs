//! `SourceRepository` over Legado MCP tools (get/save/disable/delete).

use std::sync::Arc;

use serde_json::{json, Value};
use source_ports::SourceRepository;
use source_types::{BookSource, PortError, SourceKey};

use crate::client::McpClient;

const DISABLE_TAG: &str = "网站失效";

/// MCP-backed source CRUD. URL trim / fragment variants match `mcp_client.get_source`.
pub struct McpSourceRepository {
    client: Arc<McpClient>,
    ready: std::sync::OnceLock<()>,
}

impl McpSourceRepository {
    pub fn new(client: Arc<McpClient>) -> Self {
        Self {
            client,
            ready: std::sync::OnceLock::new(),
        }
    }

    fn ensure_ready(&self) -> Result<(), PortError> {
        if self.ready.get().is_some() {
            return Ok(());
        }
        self.client.ensure_session()?;
        let _ = self.ready.set(());
        Ok(())
    }

    fn get_raw(&self, url: &str) -> Result<BookSource, PortError> {
        self.ensure_ready()?;
        let mut last = String::new();
        for cand in url_candidates(url) {
            let result = self
                .client
                .tools_call("get_source", json!({ "url": cand }))?;
            let raw = McpClient::extract_text(&result);
            last = raw.clone();
            let data = McpClient::parse_json_text(&raw);
            if let Some(src) = coerce_source(&data) {
                return Ok(src);
            }
        }
        Err(PortError::Permanent(format!(
            "unexpected get_source payload: {}",
            trunc(&last, 300)
        )))
    }
}

impl SourceRepository for McpSourceRepository {
    fn get(&self, key: &SourceKey) -> Result<BookSource, PortError> {
        self.get_raw(key.as_str())
    }

    fn save(&self, source: &BookSource) -> Result<(), PortError> {
        self.ensure_ready()?;
        let payload = serde_json::to_string(source.as_value()).map_err(|e| {
            PortError::ContractViolation(format!("serialize source: {e}"))
        })?;
        let _ = self.client.tools_call(
            "save_source",
            json!({
                "source": payload,
                "preserveEnabled": true,
                "preserveGroup": true,
            }),
        )?;
        Ok(())
    }

    fn disable(&self, key: &SourceKey) -> Result<(), PortError> {
        let mut value = self.get(key)?.into_value();
        apply_disable(&mut value, DISABLE_TAG);
        self.ensure_ready()?;
        let payload = serde_json::to_string(&value).map_err(|e| {
            PortError::ContractViolation(format!("serialize source: {e}"))
        })?;
        let _ = self.client.tools_call(
            "save_source",
            json!({
                "source": payload,
                "preserveEnabled": false,
                "preserveGroup": false,
            }),
        )?;
        Ok(())
    }

    fn delete(&self, keys: &[SourceKey]) -> Result<(), PortError> {
        self.ensure_ready()?;
        let urls: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
        let _ = self
            .client
            .tools_call("delete_sources", json!({ "urls": urls }))?;
        Ok(())
    }
}

fn coerce_source(data: &Value) -> Option<BookSource> {
    if data.get("bookSourceUrl").is_some() {
        return Some(BookSource::new(data.clone()));
    }
    if let Some(inner) = data.get("data") {
        if inner.get("bookSourceUrl").is_some() {
            return Some(BookSource::new(inner.clone()));
        }
    }
    None
}

fn apply_disable(value: &mut Value, tag: &str) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("enabled".into(), json!(false));
    let group = obj
        .get("bookSourceGroup")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut parts: Vec<String> = group
        .replace('，', ",")
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    if !parts.iter().any(|p| p == tag) {
        parts.push(tag.to_string());
    }
    obj.insert("bookSourceGroup".into(), json!(parts.join(",")));
}

/// Candidates matching Python `get_source` retry list.
pub fn url_candidates(book_source_url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<String>, s: String| {
        if !s.is_empty() && !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    };
    push(&mut out, book_source_url.to_string());
    let trimmed = book_source_url.trim();
    push(&mut out, trimmed.to_string());
    if !trimmed.is_empty() {
        push(&mut out, format!(" {trimmed}"));
    }
    let base = trimmed.split('#').next().unwrap_or(trimmed).trim();
    push(&mut out, base.to_string());
    let no_slash = base.trim_end_matches('/').to_string();
    push(&mut out, no_slash.clone());
    if !no_slash.is_empty() {
        push(&mut out, format!("{no_slash}/"));
    }
    out
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
    fn url_candidates_trim_and_fragment() {
        let c = url_candidates("  https://a.example/#tag  ");
        assert!(c.iter().any(|x| x == "https://a.example/#tag"));
        assert!(c.iter().any(|x| x == "https://a.example/"));
        assert!(c.iter().any(|x| x == "https://a.example"));
        assert!(c.iter().any(|x| x == " https://a.example/#tag"));
    }

    #[test]
    fn apply_disable_sets_group_tag() {
        let mut v = json!({
            "bookSourceUrl": "https://a.example/",
            "enabled": true,
            "bookSourceGroup": "小说"
        });
        apply_disable(&mut v, DISABLE_TAG);
        assert_eq!(v["enabled"], false);
        assert_eq!(v["bookSourceGroup"], "小说,网站失效");
    }
}
