//! Why-wave bucket labels — parity with `repair_why_wave.bucket`.

use std::collections::BTreeMap;

use serde_json::Value;

/// Classify a why-row into a bucket string.
pub fn why_bucket(row: &Value) -> String {
    if let Some(err) = row.get("http_err").and_then(|v| v.as_str()) {
        if err.contains("404") {
            return "dead_404".into();
        }
        if err.contains("403") {
            return "blocked_403".into();
        }
        if err.contains("401") {
            return "auth_401".into();
        }
        if err.contains("451") {
            return "legal_451".into();
        }
        return "http_dead".into();
    }
    match row.get("debug_books").and_then(|v| v.as_i64()) {
        Some(0) => "alive_search_zero".into(),
        Some(n) if n > 0 => "search_ok_need_deeper".into(),
        _ => "unknown".into(),
    }
}

/// Attach `bucket` field and return counts.
pub fn annotate_why_rows(rows: &mut [Value]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows.iter_mut() {
        let b = why_bucket(row);
        *counts.entry(b.clone()).or_insert(0) += 1;
        if let Some(obj) = row.as_object_mut() {
            obj.insert("bucket".into(), Value::String(b));
        }
    }
    counts
}

/// Build report JSON: `{ "rows": [...], "buckets": {...} }`.
pub fn why_report(mut rows: Vec<Value>) -> Value {
    let buckets = annotate_why_rows(&mut rows);
    serde_json::json!({
        "rows": rows,
        "buckets": buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn buckets_cover_http_and_debug() {
        assert_eq!(
            why_bucket(&json!({"http_err": "HTTP Error 404: Not Found"})),
            "dead_404"
        );
        assert_eq!(why_bucket(&json!({"debug_books": 0})), "alive_search_zero");
        assert_eq!(
            why_bucket(&json!({"debug_books": 3})),
            "search_ok_need_deeper"
        );
        let mut rows = vec![json!({"http_err": "403"}), json!({"debug_books": 0})];
        let c = annotate_why_rows(&mut rows);
        assert_eq!(c.get("blocked_403"), Some(&1));
        assert_eq!(rows[0]["bucket"], "blocked_403");
    }
}
