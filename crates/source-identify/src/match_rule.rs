//! Evaluate a single FingerprintRule against source fields / HTML.

use regex::Regex;
use source_types::{BookSource, FingerprintMatchKind, FingerprintRule};

fn str_field(source: &BookSource, path: &str) -> Option<String> {
    let mut cur = source.as_value();
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    match cur {
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn any_selector_haystack(source: &BookSource) -> String {
    let parts = [
        str_field(source, "ruleSearch.bookList"),
        str_field(source, "ruleSearch.name"),
        str_field(source, "ruleToc.chapterList"),
        str_field(source, "ruleContent.content"),
        str_field(source, "searchUrl"),
    ];
    parts.into_iter().flatten().collect::<Vec<_>>().join("\n")
}

/// True when the rule fires on `source` and/or `html`.
pub fn rule_matches(rule: &FingerprintRule, source: &BookSource, html: &str) -> bool {
    match rule.match_kind {
        FingerprintMatchKind::SearchUrlRegex => {
            let su = str_field(source, "searchUrl").unwrap_or_default();
            Regex::new(&rule.pattern)
                .map(|re| re.is_match(&su))
                .unwrap_or(false)
        }
        FingerprintMatchKind::SelectorPresent => {
            let hay = any_selector_haystack(source);
            hay.contains(&rule.pattern) || html.contains(&rule.pattern)
        }
        FingerprintMatchKind::HeaderCharset => {
            let header = str_field(source, "header").unwrap_or_default();
            let hay = format!("{header}\n{html}");
            hay.to_ascii_lowercase()
                .contains(&rule.pattern.to_ascii_lowercase())
        }
        FingerprintMatchKind::TypeEq => {
            let ty = str_field(source, "bookSourceType").unwrap_or_else(|| "0".into());
            ty == rule.pattern.trim()
        }
        FingerprintMatchKind::HtmlRegex => Regex::new(&rule.pattern)
            .map(|re| re.is_match(html))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::FingerprintMatchKind;

    fn rule(kind: FingerprintMatchKind, pattern: &str) -> FingerprintRule {
        FingerprintRule {
            id: "t".into(),
            weight: 1.0,
            match_kind: kind,
            pattern: pattern.into(),
        }
    }

    #[test]
    fn search_url_regex() {
        let src = BookSource::new(json!({"searchUrl": "/search.php?q={{key}}"}));
        assert!(rule_matches(
            &rule(FingerprintMatchKind::SearchUrlRegex, r"search\.php\?q="),
            &src,
            ""
        ));
    }

    #[test]
    fn html_regex() {
        let src = BookSource::new(json!({}));
        assert!(rule_matches(
            &rule(FingerprintMatchKind::HtmlRegex, r"(?i)xunsearch"),
            &src,
            "Powered by Xunsearch"
        ));
    }

    #[test]
    fn type_eq() {
        let src = BookSource::new(json!({"bookSourceType": 0}));
        assert!(rule_matches(
            &rule(FingerprintMatchKind::TypeEq, "0"),
            &src,
            ""
        ));
    }
}
