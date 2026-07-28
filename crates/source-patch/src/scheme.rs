//! Normalize scheme-less bookSourceUrl / searchUrl / exploreUrl (Python deep_loop).

use source_types::BookSource;

/// Prepend `http://` when a URL-like field is missing a scheme.
/// Returns change notes (empty if nothing changed).
pub fn normalize_source_schemes(source: &mut BookSource) -> Vec<String> {
    let mut notes = Vec::new();
    let root = source.as_value_mut();
    let Some(obj) = root.as_object_mut() else {
        return notes;
    };

    if let Some(bsu) = obj.get("bookSourceUrl").and_then(|v| v.as_str()) {
        let trimmed = bsu.trim();
        let head = trimmed.split("##").next().unwrap_or(trimmed).trim();
        if !head.is_empty() && !head.contains("://") {
            let fixed = format!("http://{}", trimmed.trim_start_matches('/'));
            obj.insert("bookSourceUrl".into(), serde_json::json!(fixed));
            notes.push("scheme_http:bookSourceUrl".into());
        }
    }

    for field in ["searchUrl", "exploreUrl"] {
        let Some(val) = obj.get(field).and_then(|v| v.as_str()) else {
            continue;
        };
        let head = val.split(',').next().unwrap_or(val).trim();
        if head.is_empty() || head.contains("://") {
            continue;
        }
        if head.starts_with("www.") || head.starts_with("m.") || head.starts_with("wap.") {
            let fixed = format!("http://{}", val.trim_start_matches('/'));
            obj.insert(field.to_string(), serde_json::json!(fixed));
            notes.push(format!("scheme_http:{field}"));
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adds_http_to_www() {
        let mut src = BookSource::new(json!({
            "bookSourceUrl": "www.example.com",
            "searchUrl": "www.example.com/search?q={{key}}"
        }));
        let n = normalize_source_schemes(&mut src);
        assert!(n.iter().any(|x| x.contains("bookSourceUrl")));
        assert!(src.as_value()["bookSourceUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://"));
        assert!(src.as_value()["searchUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://"));
    }
}
