//! Create helpers — route through registry / GenericForm (§11.5 stub).

use source_types::{AdapterOutcome, PatchPlan, SiteFamily};

use crate::context::RepairContext;
use crate::registry::AdapterRegistry;

/// Create via identified family; Unknown → GenericForm.
pub fn create_via_registry(
    reg: &AdapterRegistry,
    mut ctx: RepairContext,
) -> AdapterOutcome<PatchPlan> {
    if ctx.family.is_unknown() {
        ctx.family = SiteFamily::new(SiteFamily::GENERIC_FORM);
    }
    reg.create(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::{BookSource, SourceKey};

    #[test]
    fn unknown_falls_back_to_generic_form() {
        let reg = AdapterRegistry::with_seed_families();
        let html = r#"<form action="/s.php"><input name="q"/></form>"#;
        let ctx = RepairContext::new(
            SourceKey::new("https://ex.com/"),
            BookSource::new(json!({})),
            SiteFamily::unknown(),
        )
        .with_html("https://ex.com/", html);
        match create_via_registry(&reg, ctx) {
            AdapterOutcome::Plan(p) => {
                assert_eq!(p.family.as_str(), SiteFamily::GENERIC_FORM);
                assert_eq!(p.capability, source_types::Capability::Create);
            }
            other => panic!("expected Plan, got {other:?}"),
        }
    }
}
