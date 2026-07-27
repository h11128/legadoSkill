//! Structural fingerprint hash (§10.5).

use sha2::{Digest, Sha256};
use source_types::BookSource;

use crate::fields::{chapter_list, content_rule, search_book_list, search_url, source_type};

/// Normalize `searchUrl` into a shape token: path template + method + charset hints.
pub fn normalize_search_url_shape(search_url: &str) -> String {
    let trimmed = search_url.trim();
    if trimmed.is_empty() {
        return "empty".into();
    }
    let lower = trimmed.to_ascii_lowercase();
    let method = if lower.contains(",{\"method\"") || lower.contains("\"method\":\"post\"") {
        "POST"
    } else {
        "GET"
    };
    let charset = if lower.contains("charset") {
        if lower.contains("gbk") || lower.contains("gb2312") {
            "gbk"
        } else {
            "utf8"
        }
    } else {
        "default"
    };
    // Strip host; keep path + query template ({{key}} stays).
    let pathish = if let Some(idx) = trimmed.find("://") {
        let after = &trimmed[idx + 3..];
        after
            .find('/')
            .map(|i| after[i..].to_string())
            .unwrap_or_else(|| after.to_string())
    } else {
        trimmed.to_string()
    };
    // Drop header JSON suffix after first comma that starts options blob.
    let path_only = pathish
        .split_once(",{")
        .map(|(p, _)| p.to_string())
        .unwrap_or(pathish);
    format!("{method}|{charset}|{}", path_only.trim())
}

/// `sha256(join(shape, bookList, chapterList, content, type))[:16]` hex.
pub fn structural_hash(
    shape: &str,
    book_list: &str,
    chapter_list: &str,
    content: &str,
    book_source_type: &str,
) -> String {
    let joined = format!(
        "{}\n{}\n{}\n{}\n{}",
        shape.trim(),
        book_list.trim(),
        chapter_list.trim(),
        content.trim(),
        book_source_type.trim()
    );
    let digest = Sha256::digest(joined.as_bytes());
    hex::encode(&digest[..8]) // 16 hex chars
}

pub fn structural_hash_from_source(source: &BookSource) -> String {
    let shape = search_url(source)
        .map(|s| normalize_search_url_shape(&s))
        .unwrap_or_else(|| "empty".into());
    structural_hash(
        &shape,
        search_book_list(source).as_deref().unwrap_or(""),
        chapter_list(source).as_deref().unwrap_or(""),
        content_rule(source).as_deref().unwrap_or(""),
        &source_type(source),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_stable_and_16_hex() {
        let h = structural_hash("GET|default|/s?q={{key}}", ".a", ".b", "#c", "0");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        let h2 = structural_hash("GET|default|/s?q={{key}}", ".a", ".b", "#c", "0");
        assert_eq!(h, h2);
    }

    #[test]
    fn shape_strips_host() {
        let s = normalize_search_url_shape("https://Ex.com/search.php?q={{key}}");
        assert!(s.contains("/search.php?q={{key}}"));
        assert!(s.starts_with("GET|"));
    }

    #[test]
    fn from_source() {
        let src = BookSource::new(json!({
            "searchUrl": "https://a.test/search.php?q={{key}}",
            "ruleSearch": { "bookList": ".item" },
            "ruleToc": { "chapterList": ".ch" },
            "ruleContent": { "content": "#body" },
            "bookSourceType": 0
        }));
        let h = structural_hash_from_source(&src);
        assert_eq!(h.len(), 16);
    }
}
