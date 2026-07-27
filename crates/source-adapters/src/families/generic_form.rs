//! GenericForm — searchUrl from HTML form only; never invents bookList (§14.3).

use source_types::{
    AdapterOutcome, Capability, FingerprintMatchKind, FingerprintRule, NeedMoreHtml, OptimizePlan,
    PatchOp, PatchPlan, SiteFamily, Unrepairable,
};

use source_types::RepairContext;
use crate::form::search_url_from_html;
use source_ports::{CreatePlugin, FamilyPlugin, OptimizePlugin, RepairPlugin};

pub static GENERIC_FORM_RULES: &[FingerprintRule] = &[];

/// Lazy static-like rules built once via function (FingerprintRule is not const-friendly).
pub fn generic_form_rules() -> Vec<FingerprintRule> {
    vec![FingerprintRule {
        id: "html:form".into(),
        // Weight ≥ identify_min_score (2.0) so a clear <form action> identifies GenericForm
        // without silent unknown→GenericForm repair fallback.
        weight: 2.0,
        match_kind: FingerprintMatchKind::HtmlRegex,
        pattern: r"(?i)<form[^>]+action".into(),
    }]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GenericForm;

impl FamilyPlugin for GenericForm {
    fn family(&self) -> SiteFamily {
        SiteFamily::new(SiteFamily::GENERIC_FORM)
    }

    fn fingerprints(&self) -> &[FingerprintRule] {
        // Empty static slice; registry stores owned rules from generic_form_rules().
        GENERIC_FORM_RULES
    }
}

impl GenericForm {
    fn plan_from_form(&self, ctx: &RepairContext, capability: Capability) -> AdapterOutcome<PatchPlan> {
        let html = ctx.html_text();
        if html.trim().is_empty() {
            let url = ctx.primary_url().ok();
            return AdapterOutcome::NeedMoreHtml(NeedMoreHtml::new(
                url.into_iter().collect(),
                "GenericForm needs home/search HTML to read <form action>",
            ));
        }
        match search_url_from_html(&html, &ctx.base_url()) {
            Some(search_url) => {
                let url = match ctx.primary_url() {
                    Ok(u) => u,
                    Err(_) => {
                        return AdapterOutcome::Unrepairable(Unrepairable::new(
                            "invalid source url",
                            source_types::GateAction::Skip,
                        ));
                    }
                };
                let op = PatchOp::set("searchUrl", serde_json::json!(search_url))
                    .with_note("form action only; bookList not invented");
                AdapterOutcome::Plan(PatchPlan::new(
                    capability,
                    self.family(),
                    url,
                    vec![op],
                    "GenericForm: set searchUrl from HTML form",
                ))
            }
            None => AdapterOutcome::Unrepairable(Unrepairable::new(
                "no searchable <form> found in prefetched HTML",
                source_types::GateAction::Skip,
            )),
        }
    }
}

impl RepairPlugin for GenericForm {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.plan_from_form(ctx, Capability::Repair)
    }
}

impl CreatePlugin for GenericForm {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.plan_from_form(ctx, Capability::Create)
    }
}

impl OptimizePlugin for GenericForm {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::{BookSource, SourceKey};

    #[test]
    fn repair_sets_search_url_only() {
        let html = r#"<form action="/modules/article/search.php"><input name="searchkey"/></form>"#;
        let ctx = RepairContext::new(
            SourceKey::new("https://m.example.com/"),
            BookSource::new(json!({})),
            SiteFamily::new(SiteFamily::GENERIC_FORM),
        )
        .with_html("https://m.example.com/", html);
        match GenericForm.repair(&ctx) {
            AdapterOutcome::Plan(p) => {
                assert_eq!(p.ops.len(), 1);
                assert_eq!(p.ops[0].path.as_ref().unwrap().as_str(), "searchUrl");
                assert!(p
                    .ops
                    .iter()
                    .all(|o| o.path.as_ref().map(|p| p.as_str()) != Some("ruleSearch.bookList")));
            }
            other => panic!("expected Plan, got {other:?}"),
        }
    }

    #[test]
    fn need_more_html_when_empty() {
        let ctx = RepairContext::new(
            SourceKey::new("https://m.example.com/"),
            BookSource::new(json!({})),
            SiteFamily::new(SiteFamily::GENERIC_FORM),
        );
        assert!(matches!(
            GenericForm.repair(&ctx),
            AdapterOutcome::NeedMoreHtml(_)
        ));
    }
}
