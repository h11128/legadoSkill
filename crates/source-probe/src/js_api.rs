//! Detect SPA / JS search shells: data-api="…search…" (paper027 / 卧龙).

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsSearchApi {
    pub api_path: String,
    pub search_url: String,
}

fn data_api_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)data-api=["']([^"']*search[^"']*)["']"#).unwrap())
}

/// If homepage HTML embeds a JS search API path, return Legado JSON searchUrl stub.
pub fn detect_js_search_api(html: &str, base_url: &str) -> Option<JsSearchApi> {
    let cap = data_api_re().captures(html)?;
    let raw = cap.get(1)?.as_str().trim();
    if raw.is_empty() {
        return None;
    }
    let api = if raw.starts_with("http") {
        raw.to_string()
    } else if raw.starts_with('/') {
        let base = base_url.trim_end_matches('/');
        if let Ok(u) = url::Url::parse(base) {
            format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), raw)
        } else {
            format!("{base}{raw}")
        }
    } else {
        format!("{}/{}", base_url.trim_end_matches('/'), raw)
    };
    let sep = if api.contains('?') { "&" } else { "?" };
    let search_url = format!("{api}{sep}q={{{{key}}}}");
    Some(JsSearchApi {
        api_path: raw.to_string(),
        search_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_data_api() {
        let html = r#"<div data-api="/api/v1/books/search"></div>"#;
        let j = detect_js_search_api(html, "https://paper027.com/").unwrap();
        assert!(j.search_url.contains("/api/v1/books/search"));
        assert!(j.search_url.contains("q={{key}}"));
    }
}
