//! Classify book/detail/chapter URLs heuristically.

use serde_json::{json, Value};

pub fn analyze_url(url: &str) -> Value {
    let lower = url.to_lowercase();
    let kind = if lower.contains("/chapter") || lower.contains("/read/") {
        "chapter"
    } else if lower.contains("/book") || lower.contains("/novel") {
        "detail"
    } else if lower.contains("search") || lower.contains("/s.php") {
        "search"
    } else {
        "unknown"
    };
    json!({
        "url": url,
        "kind": kind,
    })
}
