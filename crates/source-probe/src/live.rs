//! Live search probe: fetch each candidate, score HTML, detect dead form endpoints.

use crate::forms::{forms_from_html, ProbeForm, SearchCandidate};
use crate::forms_js::{forms_from_js, searchish_script_srcs};
use crate::hints::{html_needs_gbk, materialize_search_url};
use crate::rank::{form_endpoints_dead, pick_best, rank_with_html, ProbeBest, RankedCandidate};
use crate::{dedupe_forms, probe_search_from_forms, ProbeResult};
use serde::{Deserialize, Serialize};
use url::Url;

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
    if let Ok(u) = Url::parse(base.trim_end_matches('/')) {
        if t.starts_with('/') {
            return format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), t);
        }
    }
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        t.trim_start_matches('/')
    )
}

fn is_post_template(su: &str) -> bool {
    su.contains("\"method\"") && su.to_ascii_lowercase().contains("post")
}

fn collect_forms(home_html: &str, base_url: &str, fetch: &FetchFn<'_>) -> Vec<ProbeForm> {
    let mut forms = forms_from_html(home_html, base_url);
    forms.extend(forms_from_js(home_html, base_url));
    for src in searchish_script_srcs(home_html) {
        let abs = absolutize(&src, base_url);
        if let Some((_, js)) = fetch(&abs) {
            forms.extend(forms_from_js(&js, base_url));
        }
    }
    dedupe_forms(&mut forms);
    forms
}

/// Live-rank up to `max_fetch` candidates (forms first). Fetches searchish JS shells.
pub fn probe_search_live(
    home_html: &str,
    base_url: &str,
    keyword: &str,
    fetch: &FetchFn<'_>,
    max_fetch: usize,
) -> LiveProbeResult {
    let forms = collect_forms(home_html, base_url, fetch);
    let offline = probe_search_from_forms(forms, keyword);
    let gbk = html_needs_gbk(home_html);
    let mut pages: Vec<(String, String, u16)> = Vec::new();

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
            continue;
        }
        let abs = absolutize(&c.search_url, base_url);
        let fetch_url = materialize_search_url(&abs, keyword, gbk);
        match fetch(&fetch_url) {
            Some((status, body)) => pages.push((c.search_url.clone(), body, status)),
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

    #[test]
    fn live_merges_js_script_form() {
        let home = r#"<script src="/js/top.js"></script>"#;
        let fetch = |url: &str| {
            if url.contains("top.js") {
                Some((
                    200u16,
                    r#"document.writeln("<form action='/modules/article/search.php' method='post'>");"#
                        .into(),
                ))
            } else {
                None
            }
        };
        let live = probe_search_live(home, "https://m.ex.com/", "我的", &fetch, 4);
        assert!(
            live.offline
                .forms
                .iter()
                .any(|f| f.action.contains("search.php")),
            "forms={:?}",
            live.offline.forms
        );
    }
}
