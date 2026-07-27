//! Rank searchUrl candidates (offline heuristics + optional fetched HTML).

use crate::forms::SearchCandidate;
use crate::score::score_search_html;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedCandidate {
    pub search_url: String,
    pub from: String,
    pub score: i32,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeBest {
    pub search_url: String,
    pub score: i32,
    pub signals: Vec<String>,
}

/// Score a candidate path without a live fetch (path markers + form boost).
pub fn score_candidate_path(c: &SearchCandidate) -> RankedCandidate {
    let su = c.search_url.to_ascii_lowercase();
    let mut score = 0;
    let mut signals = Vec::new();
    for (needle, w, tag) in [
        ("search.php", 5, "path_search_php"),
        ("modules/article/search", 5, "path_jieqi"),
        ("/s.php", 3, "path_s_php"),
        ("/so.php", 3, "path_so"),
        ("xunsearch", 4, "xunsearch"),
        ("keyword=", 1, "has_keyword"),
        ("searchkey=", 2, "has_searchkey"),
        ("?q=", 1, "has_q"),
    ] {
        if su.contains(needle) {
            score += w;
            signals.push(tag.into());
        }
    }
    if c.from != "common_path" {
        score += 2;
        signals.push("from_form".into());
    }
    RankedCandidate {
        search_url: c.search_url.clone(),
        from: c.from.clone(),
        score,
        signals,
    }
}

/// Rank using fetched result HTML via [`score_search_html`].
pub fn rank_with_html(
    candidates: &[SearchCandidate],
    pages: &[(String, String, u16)],
    keyword: &str,
) -> Vec<RankedCandidate> {
    let mut ranked = Vec::new();
    for c in candidates {
        let page = pages.iter().find(|(su, _, _)| su == &c.search_url);
        let (html, status) = match page {
            Some((_, h, s)) => (h.as_str(), *s),
            None => ("", 0),
        };
        let scored = if status == 0 && html.is_empty() {
            score_candidate_path(c)
        } else {
            let ps = score_search_html(html, keyword, status);
            let mut signals = ps.reasons;
            if c.from != "common_path" {
                signals.push("from_form".into());
            }
            RankedCandidate {
                search_url: c.search_url.clone(),
                from: c.from.clone(),
                score: ps.score + if c.from != "common_path" { 2 } else { 0 },
                signals,
            }
        };
        ranked.push(scored);
    }
    sort_ranked(&mut ranked);
    ranked
}

/// Offline rank (no network): path heuristics + form preference.
pub fn rank_offline(candidates: &[SearchCandidate]) -> Vec<RankedCandidate> {
    let mut ranked: Vec<_> = candidates.iter().map(score_candidate_path).collect();
    sort_ranked(&mut ranked);
    ranked
}

fn sort_ranked(ranked: &mut [RankedCandidate]) {
    ranked.sort_by(|a, b| {
        let af = if a.from != "common_path" { 1 } else { 0 };
        let bf = if b.from != "common_path" { 1 } else { 0 };
        (b.score, bf).cmp(&(a.score, af))
    });
}

/// Pick best like Python `probe_search_forms` (skip weak common_path / dead).
pub fn pick_best(ranked: &[RankedCandidate]) -> Option<ProbeBest> {
    for r in ranked {
        if r.score <= 0 && r.from == "common_path" {
            continue;
        }
        if r.score > 0 || r.from != "common_path" {
            return Some(ProbeBest {
                search_url: r.search_url.clone(),
                score: r.score,
                signals: r.signals.clone(),
            });
        }
    }
    ranked.first().filter(|r| r.score > 0).map(|r| ProbeBest {
        search_url: r.search_url.clone(),
        score: r.score,
        signals: r.signals.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_beats_common_path() {
        let cands = vec![
            SearchCandidate {
                search_url: "/search.php?q={{key}}".into(),
                from: "common_path".into(),
            },
            SearchCandidate {
                search_url: "/search.php?searchkey={{key}}&searchtype=all".into(),
                from: "html".into(),
            },
        ];
        let ranked = rank_offline(&cands);
        let best = pick_best(&ranked).unwrap();
        assert_eq!(best.search_url, cands[1].search_url);
        assert!(best.score > 0);
    }
}
