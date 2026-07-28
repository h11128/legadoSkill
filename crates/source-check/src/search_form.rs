//! HTML search form → Legado searchUrl template (Python search_wave/deep_wave).

use regex::Regex;
use std::sync::OnceLock;
use url::Url;

fn form_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<form[^>]*>([\s\S]{0,1500}?)</form>").unwrap())
}

fn search_hint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)search|keyword|key|wd|q=|sosuo").unwrap())
}

fn action_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)action=["']([^"']*)["']"#).unwrap())
}

fn name_re(name: &str) -> Regex {
    Regex::new(&format!(r#"(?i)name=["']{name}["']"#)).unwrap()
}

/// Find relative searchUrl template from homepage HTML.
pub fn find_search_action(html: &str, base: &str) -> Option<String> {
    let base_url = normalize_base(base);
    for cap in form_re().captures_iter(html) {
        let block = cap.get(0)?.as_str();
        if !search_hint_re().is_match(block) {
            continue;
        }
        let action = action_re()
            .captures(block)
            .and_then(|c| c.get(1).map(|m| m.as_str()))
            .unwrap_or("");
        let name = ["searchkey", "searchKey", "keyword", "key", "wd", "q"]
            .into_iter()
            .find(|n| name_re(n).is_match(block))
            .unwrap_or("q");
        let abs = join_base(&base_url, action);
        let path = Url::parse(&abs)
            .ok()
            .map(|u| u.path().to_string())
            .unwrap_or_else(|| "/".into());
        return Some(format!("{path}?{name}={{{{key}}}}"));
    }
    if Regex::new(r#"(?i)action=["'][^"']*search"#)
        .unwrap()
        .is_match(html)
    {
        if let Some(m) = Regex::new(r#"(?i)action=["']([^"']*search[^"']*)["']"#)
            .unwrap()
            .captures(html)
        {
            let action = m.get(1)?.as_str();
            let abs = join_base(&base_url, action);
            let path = Url::parse(&abs).ok()?.path().to_string();
            return Some(format!("{path}?q={{{{key}}}}"));
        }
    }
    if html.contains("/search?q=") {
        return Some("/search?q={{key}}".into());
    }
    None
}

fn normalize_base(base: &str) -> String {
    let b = base.split('#').next().unwrap_or(base).trim();
    if b.contains("://") {
        b.trim_end_matches('/').to_string() + "/"
    } else {
        format!("http://{}/", b.trim_start_matches('/'))
    }
}

fn join_base(base: &str, rel: &str) -> String {
    if rel.starts_with("http") {
        return rel.to_string();
    }
    let rel = rel.trim_start_matches('/');
    format!("{base}{rel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_get_form() {
        let html = r#"<form action="/search.php" method="get"><input name="q"/></form>"#;
        let a = find_search_action(html, "https://ex.com").unwrap();
        assert!(a.contains("search.php"));
        assert!(a.contains("q={{key}}"));
    }
}
