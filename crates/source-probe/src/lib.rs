//! Search probe: forms, common paths, offline/live ranking, bookList hints.

mod forms;
mod forms_js;
mod hints;
mod js_api;
mod live;
mod paths;
mod rank;
mod score;

pub use forms::{candidates_from_forms, forms_from_html, ProbeForm, SearchCandidate};
pub use forms_js::{forms_from_js, searchish_script_srcs};
pub use hints::{
    append_charset_gbk, encode_query_value, guess_booklist, html_needs_gbk, materialize_search_url,
    BookListHints,
};
pub use js_api::{detect_js_search_api, JsSearchApi};
pub use live::{probe_search_live, LiveProbeResult};
pub use paths::{common_path_candidates, COMMON_GET_TEMPLATES};
pub use rank::{
    form_endpoints_dead, pick_best, rank_offline, rank_with_html, score_candidate_path, ProbeBest,
    RankedCandidate,
};
pub use score::{score_search_html, score_search_html_with_home, ProbeScore};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeResult {
    pub forms: Vec<ProbeForm>,
    pub candidates: Vec<SearchCandidate>,
    pub best: Option<ProbeBest>,
    pub ranked: Vec<RankedCandidate>,
}

/// Build probe result from an already-collected form list.
pub fn probe_search_from_forms(forms: Vec<ProbeForm>, _keyword: &str) -> ProbeResult {
    let mut candidates = candidates_from_forms(&forms);
    candidates.extend(common_path_candidates(&candidates, 4));
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.search_url.clone()));
    let ranked = rank_offline(&candidates);
    let best = pick_best(&ranked);
    let mut candidates = candidates;
    if let Some(ref b) = best {
        if let Some(idx) = candidates.iter().position(|c| c.search_url == b.search_url) {
            let prefer = candidates.remove(idx);
            candidates.insert(0, prefer);
        }
    }
    ProbeResult {
        forms,
        candidates,
        best,
        ranked,
    }
}

/// Offline probe from homepage HTML (no network). Includes inline JS form shells.
pub fn probe_search(home_html: &str, base_url: &str, keyword: &str) -> ProbeResult {
    let mut forms = forms_from_html(home_html, base_url);
    forms.extend(forms_from_js(home_html, base_url));
    dedupe_forms(&mut forms);
    probe_search_from_forms(forms, keyword)
}

pub(crate) fn dedupe_forms(forms: &mut Vec<ProbeForm>) {
    let mut seen = std::collections::HashSet::new();
    forms.retain(|f| seen.insert(f.action.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_search_php_form() {
        let html = r#"
        <html><body>
        <form action="/search.php" method="get">
          <input type="text" name="q" />
          <button>search</button>
        </form>
        </body></html>"#;
        let r = probe_search(html, "https://example.com/", "我的");
        assert!(!r.forms.is_empty());
        assert!(r.best.is_some());
        let best = r.best.unwrap();
        assert!(best.search_url.contains("search.php"));
        assert!(best.search_url.contains("q={{key}}"));
    }
}
