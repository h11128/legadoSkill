//! JieqiMobile family stub — `#sitebox dl` / search.php.

use source_types::{
    AdapterOutcome, Capability, FingerprintMatchKind, FingerprintRule, Layer, NeedMoreHtml,
    OptimizePlan, PatchOp, PatchPlan, SiteFamily, Unrepairable,
};

use crate::context::RepairContext;
use crate::form::search_url_from_html;
use crate::traits::{CreatePlugin, FamilyPlugin, OptimizePlugin, RepairPlugin};

pub static JIEQI_MOBILE_RULES: &[FingerprintRule] = &[];

pub fn jieqi_mobile_rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule {
            id: "search:jieqi_modules".into(),
            weight: 2.0,
            match_kind: FingerprintMatchKind::SearchUrlRegex,
            pattern: r"modules/article/search\.php".into(),
        },
        FingerprintRule {
            id: "list:sitebox_dl".into(),
            weight: 2.0,
            match_kind: FingerprintMatchKind::SelectorPresent,
            pattern: "#sitebox".into(),
        },
        FingerprintRule {
            id: "html:sitebox".into(),
            weight: 1.5,
            match_kind: FingerprintMatchKind::HtmlRegex,
            pattern: r#"(?i)id=["']sitebox["']"#.into(),
        },
    ]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct JieqiMobile;

impl FamilyPlugin for JieqiMobile {
    fn family(&self) -> SiteFamily {
        SiteFamily::new(SiteFamily::JIEQI_MOBILE)
    }
    fn fingerprints(&self) -> &[FingerprintRule] {
        JIEQI_MOBILE_RULES
    }
}

impl JieqiMobile {
    fn propose(&self, ctx: &RepairContext, capability: Capability) -> AdapterOutcome<PatchPlan> {
        let html = ctx.html_text();
        if html.trim().is_empty() {
            return AdapterOutcome::NeedMoreHtml(NeedMoreHtml::new(
                ctx.primary_url().into_iter().collect(),
                "JieqiMobile needs result/home HTML",
            ));
        }
        let low = html.to_ascii_lowercase();
        if !(low.contains("sitebox") || low.contains("modules/article/search")) {
            return AdapterOutcome::Unrepairable(Unrepairable::new(
                "HTML lacks jieqi mobile markers",
                source_types::GateAction::Skip,
            ));
        }
        let url = match ctx.primary_url() {
            Ok(u) => u,
            Err(_) => {
                return AdapterOutcome::Unrepairable(Unrepairable::new(
                    "invalid url",
                    source_types::GateAction::Skip,
                ));
            }
        };
        let search = search_url_from_html(&html, &ctx.base_url()).unwrap_or_else(|| {
            "/modules/article/search.php?searchkey={{key}}&searchtype=all".into()
        });
        let mut plan = PatchPlan::new(
            capability,
            self.family(),
            url,
            vec![
                PatchOp::set("searchUrl", serde_json::json!(search)),
                PatchOp::set("ruleSearch.bookList", serde_json::json!("#sitebox dl")),
                PatchOp::set("ruleSearch.name", serde_json::json!("h3 a")),
                PatchOp::set("ruleSearch.bookUrl", serde_json::json!("h3 a@href")),
            ],
            "JieqiMobile stub: #sitebox dl list",
        );
        plan.expected_layer = Some(Layer::Search);
        AdapterOutcome::Plan(plan)
    }
}

impl RepairPlugin for JieqiMobile {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Repair)
    }
}

impl CreatePlugin for JieqiMobile {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Create)
    }
}

impl OptimizePlugin for JieqiMobile {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}
