//! Patch plans, merge/optimize, and adapter control unions (§3.5 / §3.7 / §14.3).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::enums::{
    Capability, GateAction, Layer, MergeStrategy, OptimizeRisk, PatchOpKind, SiteFamily,
};
use crate::identity::{BookSource, JsonPath, Url};
use crate::verify::VerifyResult;
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchOp {
    pub op: PatchOpKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<JsonPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl PatchOp {
    /// Construct a `set` op; `path` is required.
    pub fn set(path: impl Into<JsonPath>, value: impl Into<Value>) -> Self {
        Self {
            op: PatchOpKind::Set,
            path: Some(path.into()),
            value: Some(value.into()),
            from_url: None,
            to_url: None,
            note: None,
        }
    }

    pub fn delete(path: impl Into<JsonPath>) -> Self {
        Self {
            op: PatchOpKind::Delete,
            path: Some(path.into()),
            value: None,
            from_url: None,
            to_url: None,
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchPlan {
    pub schema_version: String,
    pub capability: Capability,
    pub family: SiteFamily,
    pub source_url: Url,
    pub ops: Vec<PatchOp>,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_layer: Option<Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run_ok: Option<bool>,
}

impl PatchPlan {
    pub fn new(
        capability: Capability,
        family: SiteFamily,
        source_url: Url,
        ops: Vec<PatchOp>,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            capability,
            family,
            source_url,
            ops,
            rationale: rationale.into(),
            expected_layer: None,
            dry_run_ok: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizePlan {
    pub schema_version: String,
    pub before: BookSource,
    pub after: BookSource,
    pub changes: Vec<PatchOp>,
    pub risk: OptimizeRisk,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_verify: Option<VerifyResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_verify: Option<VerifyResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeScore {
    pub enabled: bool,
    pub last_verify_ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respond_time_ms: Option<u64>,
    pub rule_completeness: f64,
    pub total: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergePlan {
    pub schema_version: String,
    pub strategy: MergeStrategy,
    pub survivors: Vec<Url>,
    pub drop: Vec<Url>,
    pub canonical: BookSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_breakdown: Option<HashMap<String, MergeScore>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeedMoreHtml {
    pub kind: NeedMoreHtmlKind,
    pub urls: Vec<Url>,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedMoreHtmlKind {
    NeedMoreHtml,
}

impl NeedMoreHtml {
    pub fn new(urls: Vec<Url>, why: impl Into<String>) -> Self {
        Self {
            kind: NeedMoreHtmlKind::NeedMoreHtml,
            urls,
            why: why.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unrepairable {
    pub kind: UnrepairableKind,
    pub reason: String,
    pub suggest: GateAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnrepairableKind {
    Unrepairable,
}

impl Unrepairable {
    pub fn new(reason: impl Into<String>, suggest: GateAction) -> Self {
        debug_assert!(matches!(suggest, GateAction::Skip | GateAction::Disable));
        Self {
            kind: UnrepairableKind::Unrepairable,
            reason: reason.into(),
            suggest,
        }
    }
}

/// Adapter return union (§14.3).
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterOutcome<T> {
    Plan(T),
    NeedMoreHtml(NeedMoreHtml),
    Unrepairable(Unrepairable),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_op_set_path() {
        let op = PatchOp::set("searchUrl", json!("https://ex.com/s?q={{key}}"));
        assert_eq!(op.op, PatchOpKind::Set);
        assert_eq!(op.path.as_ref().unwrap().as_str(), "searchUrl");
        assert_eq!(
            op.value.as_ref().unwrap(),
            &json!("https://ex.com/s?q={{key}}")
        );
        let wire = serde_json::to_value(&op).unwrap();
        assert_eq!(wire["op"], "set");
        assert_eq!(wire["path"], "searchUrl");
        let back: PatchOp = serde_json::from_value(wire).unwrap();
        assert_eq!(back, op);
    }
}
