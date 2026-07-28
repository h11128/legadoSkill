//! Infer ruleSearch selectors + charset from HTML (search-layer repair).

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookListHints {
    pub book_list: Option<String>,
    pub book_url: Option<String>,
    pub name: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
}

fn re_charset() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)charset\s*=\s*["']?\s*(gbk|gb2312|gb18030)"#).unwrap())
}

/// True when homepage / search page declares GB family charset.
pub fn html_needs_gbk(html: &str) -> bool {
    re_charset().is_match(html)
}

/// Append Legado URL option `,{"charset":"GBK"}` when missing.
pub fn append_charset_gbk(search_url: &str) -> String {
    let s = search_url.trim();
    if s.is_empty() || s.to_ascii_lowercase().contains("charset") {
        return s.to_string();
    }
    format!("{s},{{\"charset\":\"GBK\"}}")
}

/// Percent-encode `key` for a query string (UTF-8 or GBK bytes).
pub fn encode_query_value(key: &str, gbk: bool) -> String {
    let bytes: Vec<u8> = if gbk {
        let (cow, _, _) = encoding_rs::GBK.encode(key);
        cow.into_owned()
    } else {
        key.as_bytes().to_vec()
    };
    let mut out = String::new();
    for b in bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Replace first `{{key}}` in a Legado searchUrl template (strip trailing `,JSON` options).
pub fn materialize_search_url(template: &str, key: &str, gbk: bool) -> String {
    let base = template.split(",{").next().unwrap_or(template).trim();
    let enc = encode_query_value(key, gbk);
    base.replace("{{key}}", &enc)
}

/// Guess CSS/legado selectors from a search-result HTML body.
pub fn guess_booklist(html: &str) -> BookListHints {
    let lower = html.to_ascii_lowercase();
    let mut h = BookListHints::default();

    let has_list = lower.contains("class=\"list\"") || lower.contains("class='list'");
    let has_table = lower.contains("<table");
    let has_tr_book = lower.contains("class=\"book\"") || lower.contains("tr class=\"book\"");
    let has_name = lower.contains("class=\"name\"");
    let has_cover = lower.contains("class=\"cover\"");
    let has_intro = lower.contains("class=\"intro\"");
    let has_sitebox = lower.contains("id=\"sitebox\"") || lower.contains("id='sitebox'");

    if has_sitebox {
        h.book_list = Some("#sitebox dl".into());
        h.name = Some("dt a".into());
        h.book_url = Some("dt a@href".into());
        return h;
    }

    if has_list && has_table {
        // biduju: <div class="list"><table>… (no tbody)
        h.book_list = Some("class.list@table".into());
    } else if has_list && has_tr_book {
        h.book_list = Some("class.list@tr.book".into());
    } else if has_list {
        h.book_list = Some("class.list@tr".into());
    } else if has_tr_book {
        h.book_list = Some("tr.book".into());
    }

    if has_name {
        h.name = Some("class.name@tag.a@text".into());
        h.book_url = Some("class.name@tag.a@href".into());
    }
    if has_cover {
        h.cover_url = Some("class.cover@tag.img@src".into());
    }
    if has_intro {
        h.intro = Some("class.intro@text".into());
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gbk_meta_detected() {
        assert!(html_needs_gbk(
            r#"<meta http-equiv="Content-Type" content="text/html; charset=gb2312" />"#
        ));
    }

    #[test]
    fn append_charset() {
        let u = append_charset_gbk("/search.php?keyword={{key}}");
        assert_eq!(u, "/search.php?keyword={{key}},{\"charset\":\"GBK\"}");
    }

    #[test]
    fn guess_biduju_tables() {
        let html = r#"<div class="list"><table><tr class="book"><td class="name"><a href="/t/1.html">书</a></td></tr></table></div>"#;
        let h = guess_booklist(html);
        assert_eq!(h.book_list.as_deref(), Some("class.list@table"));
        assert_eq!(h.book_url.as_deref(), Some("class.name@tag.a@href"));
    }

    #[test]
    fn encode_gbk_wod() {
        let e = encode_query_value("我的", true);
        assert_eq!(e, "%CE%D2%B5%C4");
    }
}
