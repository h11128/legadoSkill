//! MergeScore pure helpers (§10.6).

use source_types::{BookSource, MergeScore};

/// Inputs for one merge candidate (pre-ranked respond_time_score 0..1).
#[derive(Debug, Clone)]
pub struct MergeCandidateInput {
    pub enabled: bool,
    pub last_verify_ok: bool,
    pub rule_completeness: f64,
    pub respond_time_ms: Option<u64>,
    /// 1.0 if missing; else inverse-rank within group (caller supplies).
    pub respond_time_score: f64,
}

/// §10.6 weights: 50*ok + 20*enabled + 20*completeness + 10*respond_time_score.
pub fn score_merge_candidate(input: &MergeCandidateInput) -> MergeScore {
    let ok = if input.last_verify_ok { 1.0 } else { 0.0 };
    let en = if input.enabled { 1.0 } else { 0.0 };
    let completeness = input.rule_completeness.clamp(0.0, 1.0);
    let rt = input.respond_time_score.clamp(0.0, 1.0);
    let total = 50.0 * ok + 20.0 * en + 20.0 * completeness + 10.0 * rt;
    MergeScore {
        enabled: input.enabled,
        last_verify_ok: input.last_verify_ok,
        respond_time_ms: input.respond_time_ms,
        rule_completeness: completeness,
        total,
    }
}

/// Inverse-rank within group: fastest → 1.0, slowest → ~0. Missing → 1.0 (§10.6).
pub fn respond_time_scores(ms: &[Option<u64>]) -> Vec<f64> {
    let known: Vec<(usize, u64)> = ms
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|t| (i, t)))
        .collect();
    if known.is_empty() {
        return ms.iter().map(|_| 1.0).collect();
    }
    let mut order = known;
    order.sort_by_key(|(_, t)| *t);
    let n = order.len().max(1) as f64;
    let mut out = vec![1.0; ms.len()];
    for (rank, (idx, _)) in order.into_iter().enumerate() {
        // rank 0 (fastest) → 1.0; last → 1/n
        out[idx] = (n - rank as f64) / n;
    }
    out
}

/// Fraction of required repair fields that are non-empty.
pub fn rule_completeness(source: &BookSource) -> f64 {
    const PATHS: &[&str] = &[
        "searchUrl",
        "ruleSearch.bookList",
        "ruleToc.chapterList",
        "ruleContent.content",
    ];
    let mut hit = 0usize;
    for path in PATHS {
        if field_nonempty(source, path) {
            hit += 1;
        }
    }
    hit as f64 / PATHS.len() as f64
}

fn field_nonempty(source: &BookSource, path: &str) -> bool {
    let mut cur = source.as_value();
    for part in path.split('.') {
        match cur.get(part) {
            Some(v) => cur = v,
            None => return false,
        }
    }
    match cur {
        serde_json::Value::String(s) => !s.trim().is_empty(),
        serde_json::Value::Null => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_score_weights() {
        let s = score_merge_candidate(&MergeCandidateInput {
            enabled: true,
            last_verify_ok: true,
            rule_completeness: 1.0,
            respond_time_ms: Some(100),
            respond_time_score: 1.0,
        });
        assert_eq!(s.total, 100.0);
    }

    #[test]
    fn respond_time_inverse_rank() {
        let scores = respond_time_scores(&[Some(300), Some(100), None]);
        assert_eq!(scores[1], 1.0); // fastest known
        assert!(scores[0] < scores[1]);
        assert_eq!(scores[2], 1.0); // missing
    }

    #[test]
    fn completeness_counts_fields() {
        let src = BookSource::new(json!({
            "searchUrl": "/s",
            "ruleSearch": { "bookList": ".a" }
        }));
        assert!((rule_completeness(&src) - 0.5).abs() < 1e-9);
    }
}
