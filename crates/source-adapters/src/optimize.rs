//! Optimize stubs: smell-driven PatchOp proposals; rate-only rejected (§10.6 / wave rule).

use source_patch::is_rate_only_ops;
use source_types::{BookSource, OptimizePlan, OptimizeRisk, PatchOp, SCHEMA_VERSION};

/// Suggested smell ops for optimize (caller may A/B verify later).
#[derive(Debug, Clone)]
pub struct OptimizeSmellInput<'a> {
    pub before: &'a BookSource,
    /// Candidate ops (e.g. strip dead exploreUrl, align concurrentRate).
    pub changes: Vec<PatchOp>,
}

/// Build OptimizePlan only when changes are non-empty and not rate-only.
pub fn optimize_smells_plan(input: OptimizeSmellInput<'_>) -> Option<OptimizePlan> {
    if input.changes.is_empty() {
        return None;
    }
    let paths: Vec<&str> = input
        .changes
        .iter()
        .filter_map(|op| op.path.as_ref().map(|p| p.as_str()))
        .collect();
    if is_rate_only_ops(&paths) {
        return None;
    }

    let mut after = input.before.as_value().clone();
    let apply_ops: Vec<(String, Option<serde_json::Value>)> = input
        .changes
        .iter()
        .filter_map(|op| {
            let path = op.path.as_ref()?.as_str().to_string();
            Some((path, op.value.clone()))
        })
        .collect();
    if source_patch::apply_ops(&mut after, &apply_ops).is_err() {
        return None;
    }

    let risk = if input.changes.len() > 2 {
        OptimizeRisk::Medium
    } else {
        OptimizeRisk::Low
    };

    Some(OptimizePlan {
        schema_version: SCHEMA_VERSION.to_string(),
        before: input.before.clone(),
        after: BookSource::new(after),
        changes: input.changes,
        risk,
        before_verify: None,
        after_verify: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_rate_only() {
        let before = BookSource::new(json!({"concurrentRate": "1000"}));
        let plan = optimize_smells_plan(OptimizeSmellInput {
            before: &before,
            changes: vec![PatchOp::set("concurrentRate", json!("2000"))],
        });
        assert!(plan.is_none());
    }

    #[test]
    fn accepts_meaningful_change() {
        let before = BookSource::new(json!({"exploreUrl": ""}));
        let plan = optimize_smells_plan(OptimizeSmellInput {
            before: &before,
            changes: vec![PatchOp::delete("exploreUrl")],
        });
        assert!(plan.is_some());
        assert!(plan.unwrap().after.as_value().get("exploreUrl").is_none());
    }
}
