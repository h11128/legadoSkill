use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeScore {
    pub score: i32,
    pub reasons: Vec<String>,
    pub dead: bool,
}

/// Rank search HTML. Higher is better. `dead` ⇒ do not rewrite selectors.
pub fn score_search_html(html: &str, query: &str, http_status: u16) -> ProbeScore {
    let mut score = 0;
    let mut reasons: Vec<String> = Vec::new();
    let mut dead = false;

    if http_status >= 500 {
        return ProbeScore {
            score: -100,
            reasons: vec![format!("http_{http_status}")],
            dead: true,
        };
    }

    let lower = html.to_ascii_lowercase();
    for (needle, w, tag) in [
        ("id=\"sitebox\"", 5, "list_sitebox"),
        ("item fiction", 5, "list_xchina"),
        ("class=\"bookbox\"", 3, "list_bookbox"),
        ("hot_sale", 3, "list_hot_sale"),
        ("result-list", 3, "list_result"),
    ] {
        if lower.contains(needle) {
            score += w;
            reasons.push(tag.into());
        }
    }

    if !query.is_empty() && html.contains(query) {
        score += 2;
        reasons.push("query_echo".into());
    }

    if lower.contains("search.php?q=") || lower.contains("xunsearch") {
        score += 2;
        reasons.push("xunsearch_shape".into());
    }

    // Fake home: nav chrome without list markers
    let listish = reasons.iter().any(|r| r.starts_with("list_"));
    if !listish
        && (lower.contains("首页") || lower.contains(">home<") || lower.contains("nav-bar"))
        && html.len() < 8000
    {
        score -= 5;
        reasons.push("fake_home_penalty".into());
    }

    if lower.contains("验证码") || (lower.contains("password") && lower.contains("login")) {
        dead = true;
        reasons.push("wall".into());
    }

    ProbeScore {
        score,
        reasons,
        dead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_xx_dead() {
        let s = score_search_html("<html></html>", "q", 503);
        assert!(s.dead);
        assert!(s.score < 0);
    }

    #[test]
    fn sitebox_boost() {
        let html = r#"<div id="sitebox"><dl><dt>书</dt></dl><dl><dt>书2</dt></dl></div>"#;
        let s = score_search_html(html, "书", 200);
        assert!(s.score >= 5);
        assert!(!s.dead);
    }
}
