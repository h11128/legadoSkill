//! Common search path templates (Python `COMMON_GET_TEMPLATES`).

use crate::forms::SearchCandidate;

/// Tried when homepage forms miss the real endpoint.
pub const COMMON_GET_TEMPLATES: &[&str] = &[
    "/search.php?q={{key}}",
    "/search.php?keyword={{key}}",
    "/search?q={{key}}",
    "/search?keyword={{key}}",
    "/search.html?q={{key}}",
    "/s.php?q={{key}}",
    "/so.php?q={{key}}",
    "/modules/article/search.php?searchkey={{key}}&searchtype=all",
];

/// Append common-path candidates not already present (capped).
pub fn common_path_candidates(existing: &[SearchCandidate], budget: usize) -> Vec<SearchCandidate> {
    let mut seen: std::collections::HashSet<&str> =
        existing.iter().map(|c| c.search_url.as_str()).collect();
    let mut out = Vec::new();
    for tmpl in COMMON_GET_TEMPLATES {
        if out.len() >= budget {
            break;
        }
        if seen.insert(tmpl) {
            out.push(SearchCandidate {
                search_url: (*tmpl).to_string(),
                from: "common_path".into(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_caps() {
        let c = common_path_candidates(&[], 2);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].from, "common_path");
    }
}
