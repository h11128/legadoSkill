//! Domain host rewrite / book-source migrate (Python `repair_domain_migrate`).

use regex::Regex;
use serde_json::Value;
use source_types::{BookSource, TypeError};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("bad hosts old={old:?} new={new:?}")]
    BadHosts { old: String, new: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Type(#[from] TypeError),
}

/// Host variants: `www.x.com` ↔ `x.com`, longest first.
pub fn host_forms(host: &str) -> Vec<String> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let mut forms = std::collections::HashSet::new();
    forms.insert(host.clone());
    if let Some(rest) = host.strip_prefix("www.") {
        forms.insert(rest.to_string());
    } else {
        forms.insert(format!("www.{host}"));
    }
    let mut v: Vec<_> = forms.into_iter().collect();
    v.sort_by_key(|h| std::cmp::Reverse(h.len()));
    v
}

fn split_comment(url: &str) -> (String, String) {
    if let Some((base, comment)) = url.split_once("##") {
        (base.to_string(), format!("##{comment}"))
    } else {
        (url.to_string(), String::new())
    }
}

fn hostname_of(raw: &str) -> Option<String> {
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };
    Url::parse(&with_scheme)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

fn is_host_boundary(b: u8) -> bool {
    matches!(
        b,
        b':' | b'/' | b'?' | b'#' | b'"' | b'\'' | b',' | b' ' | b'\t' | b'\n' | b'\r'
    )
}

/// Replace `scheme://[www.]old_host` with `new_base` (no trailing slash).
pub fn rewrite_hosts(text: &str, old_host: &str, new_base: &str) -> String {
    let new = new_base.trim_end_matches('/');
    let mut out = text.to_string();
    for h in host_forms(old_host) {
        // `regex` crate has no look-ahead; check the next byte as host boundary.
        let re = Regex::new(&format!(r"(?i)https?://{}", regex::escape(&h)))
            .expect("host rewrite regex");
        let mut result = String::with_capacity(out.len());
        let mut last = 0;
        for m in re.find_iter(&out) {
            let end = m.end();
            let ok = end == out.len() || is_host_boundary(out.as_bytes()[end]);
            result.push_str(&out[last..m.start()]);
            if ok {
                result.push_str(new);
            } else {
                result.push_str(m.as_str());
            }
            last = end;
        }
        result.push_str(&out[last..]);
        out = result;
    }
    out
}

/// Migrate book source JSON hosts + `bookSourceUrl` (Python `migrate_payload`).
pub fn migrate_book_source(
    src: &BookSource,
    from_url: &str,
    to_url: &str,
) -> Result<BookSource, MigrateError> {
    let (old_base, old_cmt) = split_comment(from_url);
    let (new_base, new_cmt) = split_comment(to_url);
    let old_host = hostname_of(&old_base).unwrap_or_default();
    let new_host = hostname_of(&new_base).unwrap_or_default();
    if old_host.is_empty() || new_host.is_empty() {
        return Err(MigrateError::BadHosts {
            old: old_host,
            new: new_host,
        });
    }
    let blob = serde_json::to_string(src.as_value())?;
    let blob2 = rewrite_hosts(&blob, &old_host, new_base.trim_end_matches('/'));
    let mut out: Value = serde_json::from_str(&blob2)?;
    let book_url = if !new_cmt.is_empty() {
        to_url.to_string()
    } else if !old_cmt.is_empty() {
        format!("{}{old_cmt}", new_base.trim_end_matches('/'))
    } else {
        to_url.to_string()
    };
    out["bookSourceUrl"] = Value::String(book_url);
    Ok(BookSource::new(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rewrite_www_and_bare() {
        let t = r#"https://www.old.com/a http://old.com/b"#;
        let out = rewrite_hosts(t, "old.com", "https://new.com");
        assert!(!out.contains("old.com"));
        assert!(out.contains("https://new.com/a"));
        assert!(out.contains("https://new.com/b"));
    }

    #[test]
    fn migrate_preserves_comment() {
        let src = BookSource::new(json!({
            "bookSourceUrl": "http://www.old.com##cmt",
            "searchUrl": "http://www.old.com/search?q={{key}}"
        }));
        let m = migrate_book_source(&src, "http://www.old.com##cmt", "https://new.com/").unwrap();
        assert_eq!(m.as_value()["bookSourceUrl"], "https://new.com##cmt");
        assert!(m
            .search_url()
            .unwrap()
            .starts_with("https://new.com/search"));
    }
}
