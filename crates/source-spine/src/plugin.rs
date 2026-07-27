//! Family / repair plugin traits (§14.3) + identify stub + no-op plugin.

use source_types::{
    AdapterOutcome, Fingerprint, FingerprintRule, GateAction, IdentifyResult, OptimizePlan,
    PatchPlan, SiteFamily, Unrepairable, Url, SCHEMA_VERSION,
};

use crate::context::RepairContext;

/// Minimal family identity for adapter registry (§14.3).
pub trait FamilyPlugin {
    fn family(&self) -> SiteFamily;
    fn fingerprints(&self) -> &[FingerprintRule];
}

/// Propose a repair `PatchPlan` for a known (or generic) family.
pub trait RepairPlugin: FamilyPlugin {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

/// Optional create path (not required for Phase C oneshot).
pub trait CreatePlugin: FamilyPlugin {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

/// Optional optimize path — `None` means no-op (not success verify).
pub trait OptimizePlugin: FamilyPlugin {
    fn optimize(&self, ctx: &RepairContext) -> Option<OptimizePlan>;
}

/// Identify stub when `source_identify` is not wired yet (§ Phase C).
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

/// No-op repair plugin — always `Unrepairable` so spine still compiles without adapters.
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
            format!(
                "noop plugin: no adapter for family {}",
                ctx.family.as_str()
            ),
            GateAction::Skip,
        ))
    }
}

impl OptimizePlugin for NoopRepairPlugin {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}
