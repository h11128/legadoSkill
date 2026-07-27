//! Build a search-layer PatchPlan from home HTML + optional live result fetch.

use std::io::Read;

use source_probe::{
    append_charset_gbk, guess_booklist, html_needs_gbk, materialize_search_url, probe_search,
};
use source_types::{
    Capability, Layer, PatchOp, PatchPlan, SiteFamily, Url,
};

/// Fetch GET body via ureq (CLI-side HtmlFetch).
pub fn fetch_text(url: &str) -> Option<(u16, String)> {
    let resp = ureq::get(url).call().ok()?;
    let status = resp.status();
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf).ok()?;
    // try utf-8 then gbk
    let text = String::from_utf8(buf.clone()).unwrap_or_else(|_| {
        let (cow, _, _) = encoding_rs::GBK.decode(&buf);
        cow.into_owned()
    });
    Some((status, text))
}

/// Absolute-ize a searchUrl template against book source base.
fn absolutize(template: &str, base: &str) -> String {
    let t = template.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return t.to_string();
    }
    let base = base.trim_end_matches('/');
    if t.starts_with('/') {
        // need host from base
        if let Ok(u) = url::Url::parse(base) {
            return format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), t);
        }
    }
    format!("{base}/{t}")
}

/// Search-layer plan: form probe → charset if GB meta → bookList from result HTML.
pub fn build_search_layer_plan(
    source_url: &str,
    home_html: &str,
    keyword: &str,
    family: SiteFamily,
) -> Option<PatchPlan> {
    let probe = probe_search(home_html, source_url, keyword);
    let mut search_url = probe
        .best
        .as_ref()
        .map(|b| b.search_url.clone())
        .or_else(|| probe.candidates.first().map(|c| c.search_url.clone()))?;

    let gbk = html_needs_gbk(home_html);
    if gbk {
        search_url = append_charset_gbk(&search_url);
    }
    // Prefer absolute searchUrl for device
    search_url = absolutize(&search_url, source_url);

    let mut ops = vec![
        PatchOp::set("searchUrl", serde_json::json!(search_url.clone()))
            .with_note(if gbk { "form+GBK charset" } else { "form probe" }),
    ];

    // Live fetch result page to infer bookList (GBK-encoded key when needed)
    let fetch_tpl = search_url.split(",{").next().unwrap_or(&search_url);
    let fetch_url = materialize_search_url(fetch_tpl, keyword, gbk);
    eprintln!("repair: search fetch {fetch_url}");
    if let Some((status, html)) = fetch_text(&fetch_url) {
        eprintln!(
            "repair: search fetch status={status} bytes={} gbk={gbk}",
            html.len()
        );
        if status < 500 {
            let hints = guess_booklist(&html);
            if let Some(bl) = hints.book_list {
                ops.push(
                    PatchOp::set("ruleSearch.bookList", serde_json::json!(bl))
                        .with_note("from search result HTML"),
                );
            }
            if let Some(bu) = hints.book_url {
                ops.push(PatchOp::set("ruleSearch.bookUrl", serde_json::json!(bu)));
            }
            if let Some(nm) = hints.name {
                ops.push(PatchOp::set("ruleSearch.name", serde_json::json!(nm)));
            }
            if let Some(cv) = hints.cover_url {
                ops.push(PatchOp::set("ruleSearch.coverUrl", serde_json::json!(cv)));
            }
            if let Some(intro) = hints.intro {
                ops.push(PatchOp::set("ruleSearch.intro", serde_json::json!(intro)));
            }
        }
    } else {
        eprintln!("repair: search fetch failed");
    }

    if ops.len() < 2 && !gbk {
        // charset-only / searchUrl-only is weak; still apply if we at least set searchUrl
    }

    let url = Url::new(source_url).ok()?;
    let mut plan = PatchPlan::new(
        Capability::Repair,
        family,
        url,
        ops,
        "search-layer: probe form + optional GBK + bookList hints",
    );
    plan.expected_layer = Some(Layer::Search);
    Some(plan)
}
