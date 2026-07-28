//! Load/sort fail JSON|JSONL into a prioritized repair queue (`repair_queue.py`).

use std::path::Path;

use serde_json::{json, Value};
use thiserror::Error;

use crate::classify::{decide, queue_sort_key};

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type QueueResult<T> = Result<T, QueueError>;

/// Load fail items from `.jsonl` or `.json` (list / results|failed|items / walk values).
pub fn load_items(path: &Path) -> QueueResult<Vec<Value>> {
    let text = std::fs::read_to_string(path)?;
    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
        let mut items = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let row: Value = serde_json::from_str(line)?;
            if row.is_object() {
                items.push(row);
            }
        }
        return Ok(items);
    }
    let data: Value = serde_json::from_str(&text)?;
    Ok(extract_items(&data))
}

fn extract_items(data: &Value) -> Vec<Value> {
    if let Some(arr) = data.as_array() {
        return arr.iter().filter(|x| x.is_object()).cloned().collect();
    }
    if let Some(obj) = data.as_object() {
        for key in ["results", "failed", "items"] {
            if let Some(list) = obj.get(key).and_then(|v| v.as_array()) {
                return list.iter().filter(|x| x.is_object()).cloned().collect();
            }
        }
        let mut out = Vec::new();
        for v in obj.values() {
            if let Some(list) = v.as_array() {
                out.extend(list.iter().filter(|x| x.is_object()).cloned());
            }
        }
        return out;
    }
    Vec::new()
}

/// Enrich + sort + limit (Python `repair_queue.main` core).
pub fn build_fail_queue(items: &[Value], limit: usize) -> Vec<Value> {
    let mut enriched = Vec::new();
    for it in items {
        let url = it
            .get("url")
            .or_else(|| it.get("bookSourceUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if url.is_empty() {
            continue;
        }
        let fail = it
            .get("message")
            .or_else(|| it.get("fail_msg"))
            .or_else(|| it.get("group"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let d = decide(&fail, None);
        enriched.push(json!({
            "url": url,
            "name": it.get("name").or_else(|| it.get("bookSourceName")).cloned().unwrap_or(Value::Null),
            "fail_msg": fail,
            "decision": d,
            "message": fail,
        }));
    }
    enriched.sort_by_key(queue_sort_key);
    if limit == 0 {
        enriched
    } else {
        enriched.into_iter().take(limit).collect()
    }
}

pub fn write_fail_queue(out: &Path, items: &[Value]) -> QueueResult<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, serde_json::to_string_pretty(items)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_jsonl_and_sort() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("f.jsonl");
        std::fs::write(
            &p,
            r#"{"url":"https://b/","message":"未知"}
{"url":"https://a/","fail_msg":"目录失效"}
"#,
        )
        .unwrap();
        let items = load_items(&p).unwrap();
        let q = build_fail_queue(&items, 50);
        assert_eq!(q.len(), 2);
        assert_eq!(q[0]["url"], "https://a/");
        assert_eq!(q[0]["decision"]["layer"], "toc");
    }
}
