//! Rank searchUrl candidates (offline heuristics + optional fetched HTML).

use crate::forms::SearchCandidate;
use crate::score::{score_search_html_with_home, ProbeScore};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedCandidate {
    pub search_url: String,
    pub from: String,
    pub score: i32,
    pub signals: Vec<String>,
    #[serde(default)]
    pub dead: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_list_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_url_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeBest {
    pub search_url: String,
    pub score: i32,
    pub signals: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_list_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_url_hint: Option<String>,
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
        dead: false,
        book_list_hint: None,
        book_url_hint: None,
    }
}

fn from_probe_score(c: &SearchCandidate, ps: ProbeScore, form_boost: bool) -> RankedCandidate {
    let mut signals = ps.reasons;
    let mut score = ps.score;
    if form_boost && c.from != "common_path" {
        score += 2;
        signals.push("from_form".into());
    }
    RankedCandidate {
        search_url: c.search_url.clone(),
        from: c.from.clone(),
        score,
        signals,
        dead: ps.dead,
        book_list_hint: ps.book_list_hint,
        book_url_hint: ps.book_url_hint,
    }
}

/// Rank using fetched result HTML. `pages`: (search_url_template, html, status).
pub fn rank_with_html(
    candidates: &[SearchCandidate],
    pages: &[(String, String, u16)],
    keyword: &str,
    home_html: Option<&str>,
) -> Vec<RankedCandidate> {
    let mut ranked = Vec::new();
    for c in candidates {
        let page = pages.iter().find(|(su, _, _)| su == &c.search_url);
        let scored = match page {
            Some((_, h, s)) => {
                let ps = score_search_html_with_home(h, keyword, *s, home_html);
                from_probe_score(c, ps, true)
            }
            None => score_candidate_path(c),
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

/// Pick best like Python (skip weak common_path / dead).
pub fn pick_best(ranked: &[RankedCandidate]) -> Option<ProbeBest> {
    for r in ranked {
        if r.dead {
            continue;
        }
        if r.score <= 0 && r.from == "common_path" {
            continue;
        }
        if r.score > 0 || r.from != "common_path" {
            return Some(to_best(r));
        }
    }
    ranked
        .iter()
        .find(|r| !r.dead && r.score > 0)
        .map(to_best)
}

fn to_best(r: &RankedCandidate) -> ProbeBest {
    ProbeBest {
        search_url: r.search_url.clone(),
        score: r.score,
        signals: r.signals.clone(),
        book_list_hint: r.book_list_hint.clone(),
        book_url_hint: r.book_url_hint.clone(),
    }
}

/// True when every non-common_path candidate that was fetched is HTTP 5xx/dead.
pub fn form_endpoints_dead(ranked: &[RankedCandidate]) -> bool {
    let formish: Vec<_> = ranked.iter().filter(|r| r.from != "common_path").collect();
    if formish.is_empty() {
        return false;
    }
    formish.iter().all(|r| r.dead || r.score <= -50)
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
                search_url: "/search.php?keyword={{key}}".into(),
                from: "html".into(),
            },
        ];
        let ranked = rank_offline(&cands);
        let best = pick_best(&ranked).unwrap();
        assert!(best.search_url.contains("keyword"));
    }

    #[test]
    fn skip_dead_in_pick_best() {
        let ranked = vec![RankedCandidate {
            search_url: "/s".into(),
            from: "html".into(),
            score: -100,
            signals: vec![],
            dead: true,
            book_list_hint: None,
            book_url_hint: None,
        }];
        assert!(pick_best(&ranked).is_none());
    }
}
