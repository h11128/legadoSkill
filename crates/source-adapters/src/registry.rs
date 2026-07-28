//! Adapter registry: family → fingerprints + optional repair/create/optimize (§14.3).

use std::collections::HashMap;

use source_identify::{identify, FamilyRules};
use source_types::{
    AdapterOutcome, BookSource, FingerprintRule, GateAction, IdentifyResult, OptimizePlan,
    PatchPlan, RepairConfig, SiteFamily, Unrepairable, Url,
};

use source_ports::{CreatePlugin, OptimizePlugin, RepairPlugin};
use source_types::RepairContext;

use crate::families::{
    fiction_list_xchina_rules, generic_form_rules, jieqi_mobile_rules, xunsearch_pid_rules,
    FictionListXchina, GenericForm, JieqiMobile, XunsearchPid,
};

#[derive(Debug)]
struct Entry {
    family: SiteFamily,
    rules: Vec<FingerprintRule>,
}

/// Maps `SiteFamily` → fingerprints; dispatches seed plugins by id.
#[derive(Default, Debug)]
pub struct AdapterRegistry {
    entries: HashMap<String, Entry>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed GenericForm + three thin family stubs.
    pub fn with_seed_families() -> Self {
        let mut reg = Self::new();
        reg.insert(SiteFamily::GENERIC_FORM, generic_form_rules());
        reg.insert(SiteFamily::XUNSEARCH_PID, xunsearch_pid_rules());
        reg.insert(SiteFamily::JIEQI_MOBILE, jieqi_mobile_rules());
        reg.insert(SiteFamily::FICTION_LIST_XCHINA, fiction_list_xchina_rules());
        reg
    }

    fn insert(&mut self, id: &str, rules: Vec<FingerprintRule>) {
        let family = SiteFamily::new(id);
        self.entries.insert(id.to_string(), Entry { family, rules });
    }

    pub fn families(&self) -> Vec<SiteFamily> {
        let mut v: Vec<_> = self.entries.values().map(|e| e.family.clone()).collect();
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    }

    pub fn fingerprints(&self, family: &SiteFamily) -> Option<&[FingerprintRule]> {
        self.entries
            .get(family.as_str())
            .map(|e| e.rules.as_slice())
    }

    pub fn identify_source(
        &self,
        url: Url,
        source: &BookSource,
        html: &str,
        config: &RepairConfig,
    ) -> IdentifyResult {
        let owned: Vec<(SiteFamily, Vec<FingerprintRule>)> = self
            .entries
            .values()
            .map(|e| (e.family.clone(), e.rules.clone()))
            .collect();
        let refs: Vec<FamilyRules<'_>> = owned
            .iter()
            .map(|(f, r)| FamilyRules {
                family: f,
                rules: r.as_slice(),
            })
            .collect();
        identify(url, source, html, &refs, config)
    }

    pub fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        match ctx.family.as_str() {
            SiteFamily::GENERIC_FORM => GenericForm.repair(ctx),
            SiteFamily::XUNSEARCH_PID => XunsearchPid.repair(ctx),
            SiteFamily::JIEQI_MOBILE => JieqiMobile.repair(ctx),
            SiteFamily::FICTION_LIST_XCHINA => FictionListXchina.repair(ctx),
            other => AdapterOutcome::Unrepairable(Unrepairable::new(
                format!("no repair plugin for {other}"),
                GateAction::Skip,
            )),
        }
    }

    pub fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        match ctx.family.as_str() {
            SiteFamily::GENERIC_FORM => GenericForm.create(ctx),
            SiteFamily::XUNSEARCH_PID => XunsearchPid.create(ctx),
            SiteFamily::JIEQI_MOBILE => JieqiMobile.create(ctx),
            SiteFamily::FICTION_LIST_XCHINA => FictionListXchina.create(ctx),
            other => AdapterOutcome::Unrepairable(Unrepairable::new(
                format!("no create plugin for {other}"),
                GateAction::Skip,
            )),
        }
    }

    pub fn optimize(&self, ctx: &RepairContext) -> Option<OptimizePlan> {
        match ctx.family.as_str() {
            SiteFamily::GENERIC_FORM => GenericForm.optimize(ctx),
            SiteFamily::XUNSEARCH_PID => XunsearchPid.optimize(ctx),
            SiteFamily::JIEQI_MOBILE => JieqiMobile.optimize(ctx),
            SiteFamily::FICTION_LIST_XCHINA => FictionListXchina.optimize(ctx),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::SourceKey;

    #[test]
    fn seed_identifies_xunsearch() {
        let reg = AdapterRegistry::with_seed_families();
        assert_eq!(reg.families().len(), 4);
        let src = BookSource::new(json!({
            "searchUrl": "https://www.alicesw.com/search.php?q={{key}}",
            "bookSourceType": 0
        }));
        let r = reg.identify_source(
            Url::new("https://www.alicesw.com").unwrap(),
            &src,
            "xunsearch engine",
            &RepairConfig::default(),
        );
        assert_eq!(r.family.as_str(), SiteFamily::XUNSEARCH_PID);
    }

    #[test]
    fn unknown_family_does_not_silently_generic_form() {
        let reg = AdapterRegistry::with_seed_families();
        let html = r#"<form action="/s.php"><input name="q"/></form>"#;
        let ctx = RepairContext::new(
            SourceKey::new("https://m.wmp8.com/"),
            BookSource::new(json!({})),
            SiteFamily::unknown(),
        )
        .with_html("https://m.wmp8.com/", html);
        match reg.repair(&ctx) {
            AdapterOutcome::Unrepairable(u) => {
                assert!(u.reason.contains("no repair plugin"));
            }
            other => panic!("expected Unrepairable for unknown, got {other:?}"),
        }
    }

    #[test]
    fn repair_jieqi_from_html() {
        let reg = AdapterRegistry::with_seed_families();
        let html = r#"<div id="sitebox"><dl><dt><h3><a href="/1.html">t</a></h3></dt></dl></div>"#;
        let ctx = RepairContext::new(
            SourceKey::new("https://m.wmp8.com/"),
            BookSource::new(json!({})),
            SiteFamily::new(SiteFamily::JIEQI_MOBILE),
        )
        .with_html("https://m.wmp8.com/", html);
        match reg.repair(&ctx) {
            AdapterOutcome::Plan(p) => {
                assert!(p.ops.iter().any(|o| {
                    o.path.as_ref().map(|p| p.as_str()) == Some("ruleSearch.bookList")
                }));
            }
            other => panic!("expected Plan, got {other:?}"),
        }
    }
}
