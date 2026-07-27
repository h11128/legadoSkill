//! Build a search-layer PatchPlan via live probe (parity with repair_deep_loop search).

use std::io::Read;

use source_probe::{
    append_charset_gbk, detect_js_search_api, guess_booklist, materialize_search_url,
    probe_search_live, ProbeBest,
};
use source_types::{Capability, Layer, PatchOp, PatchPlan, SiteFamily, Url};

pub enum SearchPlanOutcome {
    Plan(PatchPlan),
    /// Form search endpoints returned 5xx / dead — skip, do not fake-fix.
    EndpointDead,
    None,
}

fn fetch_text(url: &str) -> Option<(u16, String)> {
    let resp = ureq::get(url).call().ok()?;
    let status = resp.status();
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf).ok()?;
    let text = String::from_utf8(buf.clone()).unwrap_or_else(|_| {
        let (cow, _, _) = encoding_rs::GBK.decode(&buf);
        cow.into_owned()
    });
    Some((status, text))
}

fn absolutize(template: &str, base: &str) -> String {
    let t = template.trim();
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

fn ops_from_best(
    best: &ProbeBest,
    source_url: &str,
    keyword: &str,
    gbk: bool,
) -> Vec<PatchOp> {
    let mut search_url = best.search_url.clone();
    if gbk {
        search_url = append_charset_gbk(&search_url);
    }
    search_url = absolutize(&search_url, source_url);

    let mut ops = vec![PatchOp::set("searchUrl", serde_json::json!(search_url.clone()))
        .with_note(if gbk {
            "live probe + GBK"
        } else {
            "live probe"
        })];

    // Prefer hints from scored HTML; else re-fetch best and guess.
    let mut bl = best.book_list_hint.clone();
    let mut bu = best.book_url_hint.clone();
    if bl.is_none() || bu.is_none() {
        let tpl = search_url.split(",{").next().unwrap_or(&search_url);
        let fetch_url = materialize_search_url(tpl, keyword, gbk);
        if let Some((st, html)) = fetch_text(&fetch_url) {
            if st < 500 {
                let g = guess_booklist(&html);
                if bl.is_none() {
                    bl = g.book_list;
                }
                if bu.is_none() {
                    bu = g.book_url;
                }
                if let Some(nm) = g.name {
                    ops.push(PatchOp::set("ruleSearch.name", serde_json::json!(nm)));
                }
                if let Some(cv) = g.cover_url {
                    ops.push(PatchOp::set("ruleSearch.coverUrl", serde_json::json!(cv)));
                }
                if let Some(intro) = g.intro {
                    ops.push(PatchOp::set("ruleSearch.intro", serde_json::json!(intro)));
                }
            }
        }
    }
    if let Some(bl) = bl {
        ops.push(PatchOp::set("ruleSearch.bookList", serde_json::json!(bl)));
    }
    if let Some(bu) = bu {
        ops.push(PatchOp::set("ruleSearch.bookUrl", serde_json::json!(bu)));
    }
    ops
}

/// Live search-layer plan. Score≥2 required (Python deep_loop parity).
pub fn build_search_layer_plan(
    source_url: &str,
    home_html: &str,
    keyword: &str,
    family: SiteFamily,
) -> SearchPlanOutcome {
    if let Some(js) = detect_js_search_api(home_html, source_url) {
        eprintln!("repair: js_search_api {}", js.api_path);
        let Ok(url) = Url::new(source_url) else {
            return SearchPlanOutcome::None;
        };
        let ops = vec![
            PatchOp::set("searchUrl", serde_json::json!(js.search_url)),
            PatchOp::set("ruleSearch.bookList", serde_json::json!("$.data.data")),
            PatchOp::set("ruleSearch.name", serde_json::json!("$.title")),
            PatchOp::set("ruleSearch.author", serde_json::json!("$.author")),
            PatchOp::set("ruleSearch.bookUrl", serde_json::json!("/book/{{$.id}}")),
        ];
        let mut plan = PatchPlan::new(
            Capability::Repair,
            family,
            url,
            ops,
            "search-layer: JS data-api shell",
        );
        plan.expected_layer = Some(Layer::Search);
        return SearchPlanOutcome::Plan(plan);
    }

    let live = probe_search_live(home_html, source_url, keyword, &fetch_text, 8);
    if live.search_endpoint_dead {
        eprintln!("repair: search_endpoint_dead");
        return SearchPlanOutcome::EndpointDead;
    }
    let Some(best) = live.best else {
        return SearchPlanOutcome::None;
    };
    eprintln!(
        "repair: live best score={} url={}",
        best.score, best.search_url
    );
    if best.score < 2 && best.signals.iter().all(|s| s != "from_form") {
        // Weak common_path only — still try form candidate with score>=0 from_form
        return SearchPlanOutcome::None;
    }
    // Allow form candidates even if live score is low but >0; require >=2 for common_path.
    if best.score < 2 {
        let is_form = live
            .ranked
            .iter()
            .find(|r| r.search_url == best.search_url)
            .map(|r| r.from != "common_path")
            .unwrap_or(false);
        if !is_form {
            return SearchPlanOutcome::None;
        }
    }

    let ops = ops_from_best(&best, source_url, keyword, live.gbk);
    if ops.is_empty() {
        return SearchPlanOutcome::None;
    }
    let Ok(url) = Url::new(source_url) else {
        return SearchPlanOutcome::None;
    };
    let mut plan = PatchPlan::new(
        Capability::Repair,
        family,
        url,
        ops,
        "search-layer: live probe rank + charset + bookList",
    );
    plan.expected_layer = Some(Layer::Search);
    SearchPlanOutcome::Plan(plan)
}
