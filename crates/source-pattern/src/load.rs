//! Load ClusterSample rows from BookSource JSON dumps.

use serde_json::Value;
use source_types::{BookSource, Url};

use crate::cluster::ClusterSample;

/// Parse a JSON document into cluster samples.
///
/// Accepts:
/// - `[ {...}, ... ]` BookSource array
/// - `{ "items": [ ... ] }`
/// - `{ "by_url": { "url": {...}, ... } }` (full payload values)
/// - a single BookSource object
pub fn samples_from_json(doc: &Value, verify_ok_default: bool) -> Vec<ClusterSample> {
    let mut out = Vec::new();
    match doc {
        Value::Array(arr) => {
            for v in arr {
                if let Some(s) = sample_from_value(v, verify_ok_default) {
                    out.push(s);
                }
            }
        }
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map.get("items") {
                for v in items {
                    if let Some(s) = sample_from_value(v, verify_ok_default) {
                        out.push(s);
                    }
                }
            } else if let Some(Value::Object(by)) = map.get("by_url") {
                for (url, v) in by {
                    if let Some(mut s) = sample_from_value(v, verify_ok_default) {
                        if let Ok(u) = Url::new(url) {
                            s.url = u;
                        }
                        out.push(s);
                    }
                }
            } else if let Some(s) = sample_from_value(doc, verify_ok_default) {
                out.push(s);
            }
        }
        _ => {}
    }
    out
}

pub fn sample_from_value(v: &Value, verify_ok: bool) -> Option<ClusterSample> {
    if !v.is_object() {
        return None;
    }
    // Meta-only index rows have no searchUrl / rule* — skip (cannot hash).
    let has_rules = v.get("searchUrl").is_some()
        || v.get("ruleSearch").is_some()
        || v.get("ruleToc").is_some()
        || v.get("ruleContent").is_some();
    if !has_rules {
        return None;
    }
    let url_raw = v
        .get("bookSourceUrl")
        .or_else(|| v.get("url"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    let url = Url::new(url_raw).ok()?;
    let ok = v
        .get("verify_ok")
        .and_then(|x| x.as_bool())
        .unwrap_or(verify_ok);
    Some(ClusterSample {
        url,
        source: BookSource::new(v.clone()),
        verify_ok: ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loads_array_and_skips_meta_only() {
        let doc = json!([
            {
                "bookSourceUrl": "https://a.example/",
                "searchUrl": "/s?q={{key}}",
                "ruleSearch": { "bookList": ".a" }
            },
            { "url": "https://b.example/", "name": "meta only", "enabled": true }
        ]);
        let s = samples_from_json(&doc, true);
        assert_eq!(s.len(), 1);
        assert!(s[0].verify_ok);
    }
}
