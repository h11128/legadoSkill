//! Extract search forms and turn them into Legado `searchUrl` candidates.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeForm {
    pub action: String,
    pub method: String,
    /// Comma-joined input names (max 8).
    pub fields: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchCandidate {
    pub search_url: String,
    /// `html` | `js` | `common_path` | …
    pub from: String,
}

fn form_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<form\b([^>]*)>([\s\S]{0,4000}?)</form>").unwrap())
}

fn searchish_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)search|keyword|searchkey|wd|\bq\b|\bs\b|articlename").unwrap()
    })
}

fn action_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)action=["']([^"']*)["']"#).unwrap())
}

fn method_post_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)method=["']post["']"#).unwrap())
}

fn input_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)<input[^>]+name=["']([^"']+)["']"#).unwrap())
}

/// Parse search-related `<form>` blocks (Python `forms_from_html`).
pub fn forms_from_html(html: &str, base: &str) -> Vec<ProbeForm> {
    let base_url = Url::parse(base).ok();
    let mut out = Vec::new();
    for caps in form_re().captures_iter(html) {
        let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let body = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let blob = format!("{attrs}{body}");
        if !searchish_re().is_match(&blob) {
            continue;
        }
        let action_raw = action_re()
            .captures(attrs)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_default();
        let action = join_url(base_url.as_ref(), &action_raw);
        let method = if method_post_re().is_match(attrs) {
            "POST"
        } else {
            "GET"
        };
        let fields: Vec<&str> = input_name_re()
            .captures_iter(body)
            .filter_map(|c| c.get(1).map(|m| m.as_str()))
            .take(8)
            .collect();
        out.push(ProbeForm {
            action,
            method: method.into(),
            fields: fields.join(","),
        });
    }
    out
}

fn join_url(base: Option<&Url>, rel: &str) -> String {
    match base {
        Some(b) => b.join(rel).map(|u| u.to_string()).unwrap_or_else(|_| {
            if rel.is_empty() {
                b.to_string()
            } else {
                rel.to_string()
            }
        }),
        None => rel.to_string(),
    }
}

fn path_of(action: &str) -> String {
    Url::parse(action)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| action.to_string())
}

const SEARCH_FIELDS: &[&str] = &["searchkey", "keyword", "keyboard", "q", "wd", "s"];

/// Build Legado `searchUrl` templates from forms (Python `_candidates_from_forms`).
pub fn candidates_from_forms(forms: &[ProbeForm]) -> Vec<SearchCandidate> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for f in forms {
        if !seen.insert(f.action.clone()) {
            continue;
        }
        if let Some(c) = candidate_from_form(f) {
            candidates.push(c);
        }
    }
    candidates
}

fn candidate_from_form(f: &ProbeForm) -> Option<SearchCandidate> {
    let action = &f.action;
    let fields = f.fields.to_ascii_lowercase();
    let field_names: Vec<&str> = fields.split(',').filter(|s| !s.is_empty()).collect();
    let path = path_of(action);
    let mut rel = if path.starts_with('/') {
        path
    } else {
        format!("/{}", path.trim_start_matches('/'))
    };

    // Prefer real form field (biduju: keyword; jieqi: searchkey).
    let field = SEARCH_FIELDS
        .iter()
        .find(|c| field_names.iter().any(|f| f == *c))
        .copied()
        .unwrap_or_else(|| {
            if action.contains("modules/article/search") {
                "searchkey"
            } else {
                "keyword"
            }
        });

    if action.contains("search.php") || action.contains("modules/article/search") {
        let use_post =
            f.method.eq_ignore_ascii_case("POST") || action.contains("modules/article/search");
        let su = if use_post {
            format!(
                "{rel},{{\n  \"method\": \"POST\",\n  \"body\": \"{field}={{{{key}}}}&searchtype=all\"\n}}"
            )
        } else if field == "searchkey" {
            format!("{rel}?searchkey={{{{key}}}}&searchtype=all")
        } else {
            format!("{rel}?{field}={{{{key}}}}")
        };
        return Some(SearchCandidate {
            search_url: su,
            from: cand_from(f),
        });
    }

    if !field_names.iter().any(|n| SEARCH_FIELDS.contains(n)) {
        return None;
    }

    let mut extra = String::new();
    if action.contains("/e/sch") || field == "keyboard" {
        extra.push_str("&show=title&tempid=1&classid=0");
    }

    if f.method.eq_ignore_ascii_case("POST") {
        let mut body = format!("{field}={{{{key}}}}");
        if fields.contains("searchtype") {
            body.push_str("&searchtype=all");
        }
        if field_names.contains(&"t") && fields.contains("searchkey") {
            body.push_str("&t=1");
        }
        return Some(SearchCandidate {
            search_url: format!("{rel},{{\n  \"method\": \"POST\",\n  \"body\": \"{body}\"\n}}"),
            from: cand_from(f),
        });
    }

    if action.starts_with("http") {
        if let Ok(u) = Url::parse(action) {
            rel = u.path().to_string();
            if let Some(q) = u.query() {
                rel = format!("{rel}?{q}");
            }
        }
    }
    let sep = if rel.contains('?') { "&" } else { "?" };
    Some(SearchCandidate {
        search_url: format!("{rel}{sep}{field}={{{{key}}}}{extra}"),
        from: cand_from(f),
    })
}

fn cand_from(f: &ProbeForm) -> String {
    if f.fields.contains("from_js") {
        "js".into()
    } else {
        "html".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_search_php_form() {
        let html = r#"<form action="/search.php" method="get">
          <input name="q" type="text"/>
        </form>"#;
        let forms = forms_from_html(html, "https://ex.com/");
        assert_eq!(forms.len(), 1);
        assert!(forms[0].action.contains("/search.php"));
        assert_eq!(forms[0].method, "GET");
        assert!(forms[0].fields.contains('q'));
        let cands = candidates_from_forms(&forms);
        assert!(!cands.is_empty());
        assert!(cands[0].search_url.contains("search.php"));
        assert!(cands[0].search_url.contains("q={{key}}"));
    }

    #[test]
    fn search_php_uses_keyword_field() {
        let html = r#"<form action="http://www.biduju.net/search.php" method="get">
          <input name="keyword" type="text"/>
          <input name="submit" type="submit"/>
        </form>"#;
        let forms = forms_from_html(html, "http://www.biduju.net/");
        let cands = candidates_from_forms(&forms);
        assert!(!cands.is_empty());
        assert!(
            cands[0].search_url.contains("keyword={{key}}"),
            "got {}",
            cands[0].search_url
        );
        assert!(!cands[0].search_url.contains("searchkey="));
    }
}
