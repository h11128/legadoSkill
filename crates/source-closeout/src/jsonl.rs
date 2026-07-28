//! JSONL read helpers.

use std::path::Path;

use serde_json::Value;

pub type JsonRow = Value;

pub fn read_jsonl(path: &Path) -> Vec<JsonRow> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}
