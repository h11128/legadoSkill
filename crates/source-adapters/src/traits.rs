//! Adapter ISP traits (§14.3) — do not force create+repair+optimize on every family.

use source_types::{
    AdapterOutcome, FingerprintRule, OptimizePlan, PatchPlan, SiteFamily,
};

use crate::context::RepairContext;

pub trait FamilyPlugin {
    fn family(&self) -> SiteFamily;
    fn fingerprints(&self) -> &[FingerprintRule];
}

pub trait RepairPlugin: FamilyPlugin {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

pub trait CreatePlugin: FamilyPlugin {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan>;
}

pub trait OptimizePlugin: FamilyPlugin {
    fn optimize(&self, ctx: &RepairContext) -> Option<OptimizePlan>;
}
