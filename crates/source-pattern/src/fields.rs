//! Dotted BookSource field readers used by hash / cluster.

use serde_json::Value;
use source_types::BookSource;

fn str_at(root: &Value, path: &str) -> Option<String> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    match cur {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn search_url(source: &BookSource) -> Option<String> {
    str_at(source.as_value(), "searchUrl")
}

pub fn search_book_list(source: &BookSource) -> Option<String> {
    str_at(source.as_value(), "ruleSearch.bookList")
}

pub fn chapter_list(source: &BookSource) -> Option<String> {
    str_at(source.as_value(), "ruleToc.chapterList")
}

pub fn content_rule(source: &BookSource) -> Option<String> {
    str_at(source.as_value(), "ruleContent.content")
}

pub fn source_type(source: &BookSource) -> String {
    str_at(source.as_value(), "bookSourceType").unwrap_or_else(|| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_nested() {
        let s = BookSource::new(json!({
            "searchUrl": " /s?q={{key}} ",
            "ruleSearch": { "bookList": ".item" },
            "bookSourceType": 0
        }));
        assert_eq!(search_url(&s).as_deref(), Some("/s?q={{key}}"));
        assert_eq!(search_book_list(&s).as_deref(), Some(".item"));
        assert_eq!(source_type(&s), "0");
    }
}
