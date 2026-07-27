//! Family / repair plugin traits + IdentifyPort (§14.2–14.3).

use source_types::{
    AdapterOutcome, BookSource, FingerprintRule, IdentifyResult, OptimizePlan, PatchPlan,
    RepairConfig, RepairContext, SiteFamily, Url,
};

/// Minimal family identity for adapter registry (§14.3).
pub trait FamilyPlugin {
    fn family(&self) -> SiteFamily;
    fn fingerprints(&self) -> &[FingerprintRule];
}

/// Propose a repair `PatchPlan` for a known (or generic) family.
pub trait RepairPlugin: FamilyPlugin {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

/// Optional create path.
pub trait CreatePlugin: FamilyPlugin {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

/// Optional optimize path — `None` means no-op (not success verify).
pub trait OptimizePlugin: FamilyPlugin {
    fn optimize(&self, ctx: &RepairContext) -> Option<OptimizePlan>;
}

/// Fingerprint identify without coupling spine → adapters (§14.2).
pub trait IdentifyPort {
    fn identify(
        &self,
        url: Url,
        source: &BookSource,
        html: &str,
        config: &RepairConfig,
    ) -> IdentifyResult;
}
