//! Extract search forms from JS shells (jieqi `document.writeln` / wap.top.js).

use crate::forms::ProbeForm;
use regex::Regex;
use std::sync::OnceLock;
use url::Url;

fn action_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)action\\?=['"]([^'"]*search[^'"]*)['"]|action=['"]([^'"]*search[^'"]*)['"]|action=\\'([^\\']*search[^\\']*)\\'"#,
        )
        .unwrap()
    })
}

fn method_post_near_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)method\\?=['"]post['"]|method=['"]post['"]"#).unwrap()
    })
}

/// Parse search forms embedded in JS (Python `forms_from_js`).
pub fn forms_from_js(js: &str, base: &str) -> Vec<ProbeForm> {
    let mut out = Vec::new();
    let base_url = Url::parse(base).ok();
    for m in action_re().captures_iter(js) {
        let action_raw = m
            .iter()
            .skip(1)
            .flatten()
            .map(|c| c.as_str())
            .find(|s| !s.is_empty())
            .unwrap_or("");
        if action_raw.is_empty() {
            continue;
        }
        let action_raw = action_raw.replace("\\/", "/");
        let start = m.get(0).map(|x| x.start()).unwrap_or(0);
        let window = &js[start.saturating_sub(80)..(start + 200).min(js.len())];
        let method = if method_post_near_re().is_match(window) {
            "POST"
        } else {
            "GET"
        };
        let action = join_url(base_url.as_ref(), &action_raw);
        out.push(ProbeForm {
            action,
            method: method.into(),
            fields: "from_js".into(),
        });
    }
    if out.is_empty() && js.contains("/modules/article/search.php") {
        out.push(ProbeForm {
            action: join_url(base_url.as_ref(), "/modules/article/search.php"),
            method: "POST".into(),
            fields: "searchkey,searchtype".into(),
        });
    }
    out
}

fn join_url(base: Option<&Url>, rel: &str) -> String {
    match base {
        Some(b) => b.join(rel).map(|u| u.to_string()).unwrap_or_else(|_| rel.to_string()),
        None => rel.to_string(),
    }
}

/// Script `src` candidates likely to contain search form shells.
pub fn searchish_script_srcs(html: &str) -> Vec<String> {
    let re = Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#).ok();
    let Some(re) = re else {
        return Vec::new();
    };
    let filter = Regex::new(r"(?i)top|search|wap|main|header").ok();
    re.captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .filter(|src| filter.as_ref().map(|f| f.is_match(src)).unwrap_or(true))
        .take(6)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_writeln_action() {
        let js = r#"document.writeln("<form action='/modules/article/search.php' method='post'>");"#;
        let forms = forms_from_js(js, "https://m.ex.com/");
        assert!(!forms.is_empty());
        assert!(forms[0].action.contains("search.php"));
        assert_eq!(forms[0].method, "POST");
    }

    #[test]
    fn jieqi_hint_without_action() {
        let js = "var u='/modules/article/search.php';";
        let forms = forms_from_js(js, "https://m.ex.com/");
        assert_eq!(forms.len(), 1);
        assert!(forms[0].fields.contains("searchkey"));
    }
}
