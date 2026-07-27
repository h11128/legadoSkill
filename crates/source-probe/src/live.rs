//! Live search probe: fetch each candidate, score HTML, detect dead form endpoints.

use crate::forms::SearchCandidate;
use crate::hints::{html_needs_gbk, materialize_search_url};
use crate::rank::{form_endpoints_dead, pick_best, rank_with_html, ProbeBest, RankedCandidate};
use crate::{probe_search, ProbeResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveProbeResult {
    pub offline: ProbeResult,
    pub ranked: Vec<RankedCandidate>,
    pub best: Option<ProbeBest>,
    pub search_endpoint_dead: bool,
    pub gbk: bool,
}

/// Fetch callback: absolute URL → (status, body text).
pub type FetchFn<'a> = dyn Fn(&str) -> Option<(u16, String)> + 'a;

fn absolutize(template: &str, base: &str) -> String {
    let t = template.split(",{").next().unwrap_or(template).trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return t.to_string();
    }
    if let Ok(u) = url::Url::parse(base.trim_end_matches('/')) {
        if t.starts_with('/') {
            return format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), t);
        }
    }
    format!("{}/{}", base.trim_end_matches('/'), t.trim_start_matches('/'))
}

fn is_post_template(su: &str) -> bool {
    su.contains("\"method\"") && su.to_ascii_lowercase().contains("post")
}

/// Live-rank up to `max_fetch` candidates (forms first).
pub fn probe_search_live(
    home_html: &str,
    base_url: &str,
    keyword: &str,
    fetch: &FetchFn<'_>,
    max_fetch: usize,
) -> LiveProbeResult {
    let offline = probe_search(home_html, base_url, keyword);
    let gbk = html_needs_gbk(home_html);
    let mut pages: Vec<(String, String, u16)> = Vec::new();

    // Prefer form candidates for fetch order
    let mut ordered: Vec<&SearchCandidate> = offline
        .candidates
        .iter()
        .filter(|c| c.from != "common_path")
        .collect();
    ordered.extend(
        offline
            .candidates
            .iter()
            .filter(|c| c.from == "common_path"),
    );

    for c in ordered.into_iter().take(max_fetch.max(1)) {
        if is_post_template(&c.search_url) {
            // POST body templates: skip live GET; leave offline score
            continue;
        }
        let abs = absolutize(&c.search_url, base_url);
        let fetch_url = materialize_search_url(&abs, keyword, gbk);
        match fetch(&fetch_url) {
            Some((status, body)) => pages.push((c.search_url.clone(), body, status)),
            // Fetch failed (TLS/CF/reset): treat as dead page so offline path score cannot win.
            None => pages.push((c.search_url.clone(), String::new(), 599)),
        }
    }

    let ranked = rank_with_html(&offline.candidates, &pages, keyword, Some(home_html));
    let dead = form_endpoints_dead(&ranked);
    let best = if dead { None } else { pick_best(&ranked) };

    LiveProbeResult {
        offline,
        ranked,
        best,
        search_endpoint_dead: dead,
        gbk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_picks_keyword_page_with_tables() {
        let home = r#"<meta charset=gb2312>
        <form action="/search.php" method="get"><input name="keyword"/></form>"#;
        let fetch = |url: &str| {
            if url.contains("keyword=") {
                Some((
                    200u16,
                    r#"<div class="list"><table><tr class="book"><td class="name"><a href="/t/1.html">书</a></td></tr></table></div>"#
                        .to_string(),
                ))
            } else {
                Some((200, "<title>没找到你需要的页面</title>".into()))
            }
        };
        let live = probe_search_live(home, "http://ex.com/", "我的", &fetch, 6);
        assert!(!live.search_endpoint_dead);
        let best = live.best.expect("best");
        assert!(best.search_url.contains("keyword"));
        assert!(best.score > 0);
    }
}
