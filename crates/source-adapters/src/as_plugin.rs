//! AdapterRegistry as IdentifyPort + RepairPlugin for spine injection.

use source_ports::{FamilyPlugin, IdentifyPort, RepairPlugin};
use source_types::{
    AdapterOutcome, BookSource, FingerprintRule, IdentifyResult, PatchPlan, RepairConfig,
    RepairContext, SiteFamily, Url,
};

use crate::registry::AdapterRegistry;

impl IdentifyPort for AdapterRegistry {
    fn identify(
        &self,
        url: Url,
        source: &BookSource,
        html: &str,
        config: &RepairConfig,
    ) -> IdentifyResult {
        self.identify_source(url, source, html, config)
    }
}

/// Thin wrapper so registry can be passed as `dyn RepairPlugin`.
#[derive(Debug, Clone, Copy)]
pub struct RegistryRepairPlugin<'a>(pub &'a AdapterRegistry);

impl FamilyPlugin for RegistryRepairPlugin<'_> {
    fn family(&self) -> SiteFamily {
        SiteFamily::unknown()
    }

    fn fingerprints(&self) -> &[FingerprintRule] {
        &[]
    }
}

impl RepairPlugin for RegistryRepairPlugin<'_> {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.0.repair(ctx)
    }
}
