//! Search probe: forms, common paths, offline ranking.

mod forms;
mod paths;
mod rank;
mod score;

pub use forms::{candidates_from_forms, forms_from_html, ProbeForm, SearchCandidate};
pub use paths::{common_path_candidates, COMMON_GET_TEMPLATES};
pub use rank::{
    pick_best, rank_offline, rank_with_html, score_candidate_path, ProbeBest, RankedCandidate,
};
pub use score::{score_search_html, ProbeScore};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeResult {
    pub forms: Vec<ProbeForm>,
    pub candidates: Vec<SearchCandidate>,
    pub best: Option<ProbeBest>,
    pub ranked: Vec<RankedCandidate>,
}

/// Offline probe from homepage HTML (no network).
///
/// Extracts forms → Legado candidates, appends common paths, ranks by path
/// heuristics (form endpoints preferred over `common_path`).
pub fn probe_search(home_html: &str, base_url: &str, _keyword: &str) -> ProbeResult {
    let forms = forms_from_html(home_html, base_url);
    let mut candidates = candidates_from_forms(&forms);
    candidates.extend(common_path_candidates(&candidates, 4));
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.search_url.clone()));
    let ranked = rank_offline(&candidates);
    let best = pick_best(&ranked);
    // Prefer best at front of candidates list (Python behavior)
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
        assert!(r.forms[0].action.contains("/search.php"));
        assert!(r.best.is_some());
        let best = r.best.unwrap();
        assert!(best.search_url.contains("search.php"));
        assert!(best.score > 0);
        assert!(!r.ranked.is_empty());
    }
}
