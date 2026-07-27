//! Safe rule-smell fixes (Python `repair_rule_smells.py`).

use regex::Regex;
use serde_json::Value;
use source_types::BookSource;
use std::sync::OnceLock;

fn webview_single_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\{\s*'webView'\s*:\s*(true|false)\s*\}").unwrap())
}

fn webview_double_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)\{\s*"webView"\s*:\s*(true|false)\s*\}"#).unwrap())
}

fn js_baseurl_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^@?js:baseUrl$").unwrap())
}

fn class_space_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(class\.\w+)\s+a(?:\.(\d+))?(@href)$").unwrap())
}

/// Legado URL options must be JSON with double quotes: `{"webView":true}`.
pub fn fix_webview_quotes(text: &str) -> (String, bool) {
    if !text.contains("webView") {
        return (text.to_string(), false);
    }
    let new2 = webview_single_re().replace_all(text, r#"{"webView":$1}"#);
    let new2 = webview_double_re().replace_all(new2.as_ref(), |caps: &regex::Captures| {
        format!(
            r#"{{"webView":{}}}"#,
            caps.get(1)
                .map(|m| m.as_str().to_ascii_lowercase())
                .unwrap_or_else(|| "true".to_string())
        )
    });
    let changed = new2.as_ref() != text;
    (new2.into_owned(), changed)
}

/// `class.X a@href` → `class.X@tag.a@href`; drop `||@js:baseUrl`.
pub fn fix_bookurl_class_space(book_url: &str) -> (String, bool) {
    if book_url.is_empty() {
        return (book_url.to_string(), false);
    }
    let parts: Vec<&str> = book_url
        .split("||")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let mut cleaned = Vec::new();
    let mut changed = false;
    for p in parts {
        if js_baseurl_re().is_match(p) {
            changed = true;
            continue;
        }
        if let Some(caps) = class_space_re().captures(p) {
            let idx = caps
                .get(2)
                .map(|m| format!(".{}", m.as_str()))
                .unwrap_or_default();
            cleaned.push(format!(
                "{}@tag.a{}{}",
                &caps[1],
                idx,
                &caps[3]
            ));
            changed = true;
            continue;
        }
        cleaned.push(p.to_string());
    }
    if cleaned.is_empty() {
        return (book_url.to_string(), false);
    }
    let new = cleaned.join("||");
    let changed = changed || new != book_url;
    (new, changed)
}

const WEBVIEW_FIELDS: &[(&str, &[&str])] = &[
    ("ruleSearch", &["bookUrl"]),
    ("ruleBookInfo", &["tocUrl", "init"]),
    ("ruleToc", &["chapterUrl", "chapterList"]),
    ("ruleContent", &["nextContentUrl", "content"]),
];

/// Apply non-destructive learned fixes. Mutates `source`; returns change labels.
pub fn apply_safe_rule_fixes(source: &mut BookSource) -> Vec<String> {
    let mut changes = Vec::new();
    let root = source.as_value_mut();
    for &(rule_key, field_keys) in WEBVIEW_FIELDS {
        let Some(rule) = root.get_mut(rule_key) else {
            continue;
        };
        let Some(obj) = rule.as_object_mut() else {
            continue;
        };
        for fk in field_keys {
            let Some(Value::String(val)) = obj.get(*fk) else {
                continue;
            };
            if !val.contains("webView") {
                continue;
            }
            let (fixed, changed) = fix_webview_quotes(val);
            if changed {
                obj.insert((*fk).to_string(), Value::String(fixed));
                changes.push(format!("webview_quotes:{rule_key}.{fk}"));
            }
        }
    }
    if let Some(Value::String(su)) = root.get("searchUrl") {
        if su.contains("webView") {
            let (fixed, changed) = fix_webview_quotes(su);
            if changed {
                root["searchUrl"] = Value::String(fixed);
                changes.push("webview_quotes:searchUrl".into());
            }
        }
    }
    if let Some(rs) = root.get_mut("ruleSearch").and_then(|v| v.as_object_mut()) {
        if let Some(bu) = rs.get("bookUrl").and_then(|v| v.as_str()) {
            let (fixed_bu, bu_changed) = fix_bookurl_class_space(bu);
            if bu_changed {
                rs.insert("bookUrl".into(), Value::String(fixed_bu));
                changes.push("bookUrl_class_space".into());
            }
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn webview_single_quotes() {
        let (fixed, changed) = fix_webview_quotes("https://x.com/,{'webView': true}");
        assert!(changed);
        assert!(fixed.contains(r#"{"webView":true}"#));
        assert!(!fixed.contains("'webView'"));
    }

    #[test]
    fn apply_safe_fixes_search_url() {
        let mut src = BookSource::new(json!({
            "bookSourceUrl": "https://ex.com",
            "searchUrl": "/s,{'webView':true}"
        }));
        let ch = apply_safe_rule_fixes(&mut src);
        assert!(ch.iter().any(|c| c.contains("webview_quotes")));
        assert!(src.search_url().unwrap().contains(r#"{"webView":true}"#));
    }

    #[test]
    fn bookurl_class_space() {
        let (fixed, changed) = fix_bookurl_class_space("class.item a@href||@js:baseUrl");
        assert!(changed);
        assert_eq!(fixed, "class.item@tag.a@href");
    }
}
