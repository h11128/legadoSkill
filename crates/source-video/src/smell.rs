//! Light video/file smells — parity with `scripts/video_repair_one.py` `smell_video`.

use serde_json::Value;

/// Return smell ids for a media BookSource JSON object.
pub fn smell_video(source: &Value) -> Vec<String> {
    let mut smells = Vec::new();
    let bst = source.get("bookSourceType");
    let is_textish = match bst {
        None | Some(Value::Null) => true,
        Some(Value::Number(n)) => n.as_i64() == Some(0),
        Some(Value::String(s)) => s == "0" || s.is_empty(),
        _ => false,
    };
    if is_textish {
        // Approximate Python f"bookSourceType={bst!r}_should_be_video"
        let py_repr = match bst {
            None | Some(Value::Null) => "None".to_string(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) => format!("'{s}'"),
            Some(other) => other.to_string(),
        };
        smells.push(format!("bookSourceType={py_repr}_should_be_video"));
    }

    let explore_ok = source
        .get("exploreUrl")
        .map(|v| match v {
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            _ => true,
        })
        .unwrap_or(false);
    let search_ok = source
        .get("searchUrl")
        .map(|v| match v {
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            _ => true,
        })
        .unwrap_or(false);
    if !explore_ok && !search_ok {
        smells.push("no_explore_or_search".into());
    }

    let download_ok = source
        .get("ruleBookInfo")
        .filter(|bi| bi.is_object())
        .and_then(|bi| bi.get("downloadUrls"))
        .map(|d| match d {
            Value::Null => false,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        })
        .unwrap_or(false);
    if !download_ok {
        smells.push("missing_downloadUrls".into());
    }

    smells
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn smells_text_on_media_host() {
        let src = json!({
            "bookSourceType": 0,
            "searchUrl": "",
            "exploreUrl": null,
            "ruleBookInfo": {}
        });
        let s = smell_video(&src);
        assert!(s.iter().any(|x| x == "bookSourceType=0_should_be_video"));
        assert!(s.iter().any(|x| x == "no_explore_or_search"));
        assert!(s.iter().any(|x| x == "missing_downloadUrls"));
    }

    #[test]
    fn clean_video_source() {
        let src = json!({
            "bookSourceType": 4,
            "searchUrl": "/index.php/vod/search.html?wd={{key}}",
            "ruleBookInfo": { "downloadUrls": "$.list[*].url" }
        });
        assert!(smell_video(&src).is_empty());
    }
}
