//! Extract search form action → searchUrl candidate (no invented bookList).

use regex::Regex;

/// Best-effort form → Legado `searchUrl` template. Relative actions keep leading `/`.
pub fn search_url_from_html(html: &str, base: &str) -> Option<String> {
    let form_re = Regex::new(
        r#"(?is)<form[^>]*action\s*=\s*["']([^"']+)["'][^>]*>(.*?)</form>"#,
    )
    .ok()?;
    let input_re =
        Regex::new(r#"(?is)<input[^>]*name\s*=\s*["']([^"']+)["'][^>]*>"#).ok()?;

    let mut best: Option<(i32, String)> = None;
    for cap in form_re.captures_iter(html) {
        let action = cap.get(1)?.as_str().trim();
        let body = cap.get(2)?.as_str();
        let mut score = 0;
        let lower = body.to_ascii_lowercase();
        if lower.contains("search") || lower.contains("key") || lower.contains("keyword") {
            score += 2;
        }
        let mut key_name: Option<&str> = None;
        for ic in input_re.captures_iter(body) {
            let name = ic.get(1)?.as_str();
            let nl = name.to_ascii_lowercase();
            if matches!(
                nl.as_str(),
                "q" | "key" | "keyword" | "searchkey" | "search" | "wd" | "s"
            ) {
                score += 3;
                key_name = Some(name);
                break;
            }
            if key_name.is_none() && nl != "submit" && nl != "button" {
                key_name = Some(name);
            }
        }
        let name = key_name.unwrap_or("q");
        let resolved = resolve_action(action, base);
        let url = if resolved.contains('?') {
            format!("{resolved}&{name}={{{{key}}}}")
        } else {
            format!("{resolved}?{name}={{{{key}}}}")
        };
        match &best {
            Some((s, _)) if *s >= score => {}
            _ => best = Some((score, url)),
        }
    }
    best.map(|(_, u)| u)
}

fn resolve_action(action: &str, base: &str) -> String {
    let a = action.trim();
    if a.starts_with("http://") || a.starts_with("https://") {
        return a.to_string();
    }
    if a.starts_with("//") {
        let scheme = if base.starts_with("https") {
            "https:"
        } else {
            "http:"
        };
        return format!("{scheme}{a}");
    }
    if a.starts_with('/') {
        // Keep site-relative path (Legado resolves against bookSourceUrl host).
        return a.to_string();
    }
    let base_trim = base.trim_end_matches('/');
    format!("{base_trim}/{a}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_search_form() {
        let html = r#"
        <form action="/search.php" method="get">
          <input name="q" type="text"/>
          <button>go</button>
        </form>"#;
        let u = search_url_from_html(html, "https://ex.com").unwrap();
        assert_eq!(u, "/search.php?q={{key}}");
    }
}
