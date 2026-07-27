//! Auto patches from rule smells (Python `repair_patches.apply_auto_patches`).

use regex::Regex;
use serde_json::{json, Value};
use source_types::BookSource;
use std::sync::OnceLock;

use crate::smells::apply_safe_rule_fixes;

fn broad_toc_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"a@href##").unwrap())
}

fn contentish_toc_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(在线阅读|开始阅读|全文)").unwrap())
}

fn author_p_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"p\.\d+@").unwrap())
}

fn ensure_info(root: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !root.get("ruleBookInfo").map(|v| v.is_object()).unwrap_or(false) {
        root["ruleBookInfo"] = json!({});
    }
    root.get_mut("ruleBookInfo")
        .and_then(|v| v.as_object_mut())
        .expect("ruleBookInfo object")
}

fn detect_toc_smells(toc: &str, name: &str) -> Vec<&'static str> {
    let mut issues = Vec::new();
    if broad_toc_re().is_match(toc) && !toc.contains("text.") {
        issues.push("broad_a_href_regex");
    }
    if name.contains("||") && name.contains("##") {
        issues.push("fallback_mixed_with_regex");
    }
    if contentish_toc_re().is_match(toc) && toc.to_ascii_lowercase().contains("read") {
        issues.push("maybe_content_not_catalog");
    }
    issues
}

/// Return `(mutated source, change labels)`. Clears bad tocUrl smells when
/// detectable; sets default `concurrentRate`; applies safe rule fixes.
pub fn apply_auto_patches(source: &mut BookSource) -> Vec<String> {
    let mut changes = Vec::new();
    {
        let root = source.as_value_mut();
        let info = ensure_info(root);
        let toc = info
            .get("tocUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let smells = detect_toc_smells(&toc, &name);
        for issue in &smells {
            if (*issue == "broad_a_href_regex" || *issue == "maybe_content_not_catalog")
                && !toc.is_empty()
            {
                info.insert("tocUrl".into(), Value::String(String::new()));
                changes.push(format!("clear ruleBookInfo.tocUrl ({issue})"));
            } else if *issue == "fallback_mixed_with_regex" && name.contains("||") {
                let left = name.split("||").next().unwrap_or("").trim().to_string();
                info.insert("name".into(), Value::String(left));
                changes.push("ruleBookInfo.name: drop || fallback mixed with ##".into());
            }
        }
        let author = info
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if author_p_re().is_match(&author) {
            let mut fixed = author_p_re().replace_all(&author, " ").to_string();
            fixed = fixed.replace("@a@text", "@text").replace("@a", "@text");
            let fixed = Regex::new(r"\s+")
                .unwrap()
                .replace_all(&fixed, " ")
                .trim()
                .to_string();
            let fixed = if !fixed.contains('@') {
                format!("{fixed}@text")
            } else {
                fixed
            };
            info.insert("author".into(), Value::String(fixed.clone()));
            changes.push(format!("ruleBookInfo.author → {fixed}"));
        }
        let rate = root
            .get("concurrentRate")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if rate.is_empty() {
            root["concurrentRate"] = Value::String("1000".into());
            changes.push("concurrentRate → 1000".into());
        }
    }
    for label in apply_safe_rule_fixes(source) {
        changes.push(label);
    }
    // Dedup preserving order
    let mut seen = std::collections::HashSet::new();
    changes.retain(|c| seen.insert(c.clone()));
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clears_broad_toc_and_sets_rate() {
        let mut src = BookSource::new(json!({
            "bookSourceUrl": "https://ex.com",
            "ruleBookInfo": { "tocUrl": "a@href##.*/read/.*" }
        }));
        let ch = apply_auto_patches(&mut src);
        assert!(ch.iter().any(|c| c.contains("clear ruleBookInfo.tocUrl")));
        assert!(ch.iter().any(|c| c.contains("concurrentRate")));
        assert_eq!(
            src.as_value()["ruleBookInfo"]["tocUrl"].as_str(),
            Some("")
        );
        assert_eq!(src.as_value()["concurrentRate"], "1000");
    }
}
