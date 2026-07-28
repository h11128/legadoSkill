//! Decision tree + URL kind — parity with `repair_classify.py` + `layer_for_fail`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
use url::Url;

pub const LAYER_PRIORITY: &[(&str, i32)] = &[
    ("toc", 10),
    ("content", 20),
    ("search", 30),
    ("js", 40),
    ("timeout", 80),
    ("unknown", 90),
    ("skip", 100),
];

const DISABLE_HINTS: &[&str] = &[
    "域名失效",
    "网站失效",
    "下载链接为空",
    "非书源",
    "Timed out",
    "校验超时",
];

const SKIP_TAGS: &[&str] = &["域名失效", "网站失效"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub layer: String,
    pub action: String,
    pub reason: String,
    pub priority: i32,
}

pub fn layer_priority(layer: &str) -> i32 {
    LAYER_PRIORITY
        .iter()
        .find(|(k, _)| *k == layer)
        .map(|(_, p)| *p)
        .unwrap_or(90)
}

/// Python `repair_helpers.layer_for_fail`.
pub fn layer_for_fail(msg: &str) -> String {
    let m = msg;
    if SKIP_TAGS.iter().any(|t| m.contains(t)) {
        return "skip".into();
    }
    if m.contains("目录") {
        return "toc".into();
    }
    if m.contains("正文") {
        return "content".into();
    }
    if m.contains("js失效") || m.contains("EcmaError") {
        return "js".into();
    }
    if m.contains("下载链接") {
        return "skip".into();
    }
    if m.contains("搜索") || m.contains("发现") {
        return "search".into();
    }
    if m.contains("超时") || m.contains("Timed out") {
        return "timeout".into();
    }
    "unknown".into()
}

/// Python `repair_classify.decide`.
pub fn decide(fail_msg: &str, smells: Option<&[Value]>) -> Decision {
    let layer = layer_for_fail(fail_msg);
    let mut action = "fix".to_string();
    let mut reason = layer.clone();
    let disable_hint = DISABLE_HINTS.iter().any(|h| fail_msg.contains(h));
    if layer == "skip" || disable_hint {
        if fail_msg.contains("超时") || fail_msg.contains("Timed out") {
            if fail_msg.contains("域名失效") || fail_msg.contains("网站失效") {
                action = "disable".into();
                reason = "dead_host".into();
            } else {
                action = "skip".into();
                reason = "timeout_defer".into();
            }
        } else if fail_msg.contains("下载链接") {
            action = "skip".into();
            reason = "file_source".into();
        } else {
            action = "disable".into();
            reason = "dead_or_invalid".into();
        }
    }
    if layer == "toc" {
        if let Some(smells) = smells {
            let issues: Vec<&str> = smells
                .iter()
                .filter_map(|s| s.get("issue").and_then(|v| v.as_str()))
                .collect();
            if issues
                .iter()
                .any(|i| *i == "broad_a_href_regex" || *i == "maybe_content_not_catalog")
            {
                action = "auto_patch".into();
                reason = "toc_smell".into();
            }
        }
    }
    Decision {
        priority: layer_priority(&layer),
        layer,
        action,
        reason,
    }
}

/// Classify resolved toc/detail URL: homepage | content | catalog | other.
pub fn classify_resolved_url(url: &str, html: Option<&str>) -> Value {
    static RE_CHAPTER: OnceLock<Regex> = OnceLock::new();
    static RE_BOOK: OnceLock<Regex> = OnceLock::new();
    static RE_NESTED: OnceLock<Regex> = OnceLock::new();
    static RE_CATALOG: OnceLock<Regex> = OnceLock::new();
    let re_chapter = RE_CHAPTER.get_or_init(|| Regex::new(r"/\d+\.html?$").unwrap());
    let re_book = RE_BOOK.get_or_init(|| Regex::new(r"(?i)/(book|info|txt|xs|novel)/").unwrap());
    let re_nested = RE_NESTED.get_or_init(|| Regex::new(r"/\d+/\d+\.html?$").unwrap());
    let re_catalog = RE_CATALOG
        .get_or_init(|| Regex::new(r"(?i)/(read|chapter|mulu|catalog|toc)(/|$)").unwrap());

    let path = Url::parse(url)
        .ok()
        .map(|u| {
            let p = u.path().trim_end_matches('/');
            if p.is_empty() {
                "/".to_string()
            } else {
                p.to_string()
            }
        })
        .unwrap_or_else(|| "/".into());
    let mut kind = "other".to_string();
    let low_path = path.to_ascii_lowercase();
    if path == "/" || low_path == "/index" || low_path == "/index.html" || low_path == "/index.htm"
    {
        kind = "homepage".into();
    } else if re_chapter.is_match(&path) && !re_book.is_match(&path) {
        if re_nested.is_match(&path) {
            kind = "content".into();
        }
    } else if re_catalog.is_match(&path) {
        kind = "catalog".into();
    }
    if let Some(html) = html {
        let low = html.to_ascii_lowercase();
        let catalog_keys = ["chapter-list", "catalog", "mulu", "章节列表", "目录"];
        let content_keys = ["yd_text", "content_txt", "chaptercontent", "正文"];
        let catalog_hits = catalog_keys
            .iter()
            .filter(|k| low.contains(&k.to_ascii_lowercase()) || html.contains(*k))
            .count();
        let content_hits = content_keys
            .iter()
            .filter(|k| low.contains(&k.to_ascii_lowercase()))
            .count();
        if catalog_hits >= 2 && kind != "homepage" {
            kind = "catalog".into();
        } else if content_hits >= 2 && catalog_hits == 0 {
            kind = "content".into();
        }
    }
    json!({"url": url, "kind": kind, "path": path})
}

/// Sort key: (priority, url) — lower first.
pub fn queue_sort_key(item: &Value) -> (i32, String) {
    let fail = item
        .get("message")
        .or_else(|| item.get("fail_msg"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let d = decide(fail, None);
    let url = item
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    (d.priority, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_timeout_defer() {
        let d = decide("校验超时", None);
        assert_eq!(d.action, "skip");
        assert_eq!(d.reason, "timeout_defer");
    }

    #[test]
    fn decide_dead_host() {
        let d = decide("网站失效", None);
        assert_eq!(d.action, "disable");
        assert_eq!(d.layer, "skip");
    }

    #[test]
    fn classify_nested_content() {
        let v = classify_resolved_url("https://a.example/20810/1.html", None);
        assert_eq!(v["kind"], "content");
    }

    #[test]
    fn sort_toc_before_unknown() {
        let a = json!({"url":"https://b/","message":"目录失效"});
        let b = json!({"url":"https://a/","message":"weird"});
        assert!(queue_sort_key(&a) < queue_sort_key(&b));
    }
}
