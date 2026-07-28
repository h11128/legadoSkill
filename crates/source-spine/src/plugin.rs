//! No-op plugin + identify stub (real identify via IdentifyPort / adapters).

use source_ports::{FamilyPlugin, OptimizePlugin, RepairPlugin};
use source_types::{
    AdapterOutcome, Fingerprint, FingerprintRule, GateAction, IdentifyResult, OptimizePlan,
    PatchPlan, RepairContext, SiteFamily, Unrepairable, Url, SCHEMA_VERSION,
};

/// Identify stub when no `IdentifyPort` is injected.
pub fn identify_stub(ctx: &RepairContext) -> IdentifyResult {
    let url = ctx
        .source_key
        .to_url()
        .unwrap_or_else(|_| Url::new("https://invalid.local/").expect("literal"));
    IdentifyResult {
        schema_version: SCHEMA_VERSION.to_string(),
        url,
        family: SiteFamily::unknown(),
        fingerprint: Fingerprint {
            signals: vec![],
            structural_hash: "stub".into(),
            confidence: 0.0,
        },
        evidence_urls: vec![],
        score: 0.0,
        runner_up: None,
    }
}

/// No-op repair plugin — always `Unrepairable` (tests without adapters).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopRepairPlugin;

impl FamilyPlugin for NoopRepairPlugin {
    fn family(&self) -> SiteFamily {
        SiteFamily::unknown()
    }

    fn fingerprints(&self) -> &[FingerprintRule] {
        &[]
    }
}

impl RepairPlugin for NoopRepairPlugin {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        AdapterOutcome::Unrepairable(Unrepairable::new(
            format!("noop plugin: no adapter for family {}", ctx.family.as_str()),
            GateAction::Skip,
        ))
    }
}

impl OptimizePlugin for NoopRepairPlugin {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}
