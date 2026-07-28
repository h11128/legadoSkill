//! FictionListXchina family stub — `.item.fiction` / `.fiction-body`.

use source_types::{
    AdapterOutcome, Capability, FingerprintMatchKind, FingerprintRule, Layer, NeedMoreHtml,
    OptimizePlan, PatchOp, PatchPlan, SiteFamily, Unrepairable,
};

use source_ports::{CreatePlugin, FamilyPlugin, OptimizePlugin, RepairPlugin};
use source_types::RepairContext;

pub static FICTION_LIST_XCHINA_RULES: &[FingerprintRule] = &[];

pub fn fiction_list_xchina_rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule {
            id: "list:item_fiction".into(),
            weight: 2.0,
            match_kind: FingerprintMatchKind::SelectorPresent,
            pattern: ".item.fiction".into(),
        },
        FingerprintRule {
            id: "html:item_fiction".into(),
            weight: 2.0,
            match_kind: FingerprintMatchKind::HtmlRegex,
            pattern: r#"(?i)class=["'][^"']*item[^"']*fiction"#.into(),
        },
        FingerprintRule {
            id: "content:fiction_body".into(),
            weight: 1.5,
            match_kind: FingerprintMatchKind::HtmlRegex,
            pattern: r#"(?i)fiction-body"#.into(),
        },
    ]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FictionListXchina;

impl FamilyPlugin for FictionListXchina {
    fn family(&self) -> SiteFamily {
        SiteFamily::new(SiteFamily::FICTION_LIST_XCHINA)
    }
    fn fingerprints(&self) -> &[FingerprintRule] {
        FICTION_LIST_XCHINA_RULES
    }
}

impl FictionListXchina {
    fn propose(&self, ctx: &RepairContext, capability: Capability) -> AdapterOutcome<PatchPlan> {
        let html = ctx.html_text();
        if html.trim().is_empty() {
            return AdapterOutcome::NeedMoreHtml(NeedMoreHtml::new(
                ctx.primary_url().into_iter().collect(),
                "FictionListXchina needs list/content HTML",
            ));
        }
        let low = html.to_ascii_lowercase();
        if !(low.contains("item") && low.contains("fiction")) {
            return AdapterOutcome::Unrepairable(Unrepairable::new(
                "HTML lacks .item.fiction markers",
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
        let mut ops = vec![
            PatchOp::set("ruleSearch.bookList", serde_json::json!(".item.fiction")),
            PatchOp::set("ruleSearch.name", serde_json::json!(".title a, a.title")),
            PatchOp::set(
                "ruleSearch.bookUrl",
                serde_json::json!(".title a@href, a.title@href"),
            ),
        ];
        if low.contains("fiction-body") {
            ops.push(PatchOp::set(
                "ruleContent.content",
                serde_json::json!(".fiction-body"),
            ));
        }
        if low.contains("chapter-container") {
            ops.push(PatchOp::set(
                "ruleToc.chapterList",
                serde_json::json!(".chapter-container a"),
            ));
        }
        let mut plan = PatchPlan::new(
            capability,
            self.family(),
            url,
            ops,
            "FictionListXchina stub: .item.fiction list",
        );
        plan.expected_layer = Some(Layer::Search);
        AdapterOutcome::Plan(plan)
    }
}

impl RepairPlugin for FictionListXchina {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Repair)
    }
}

impl CreatePlugin for FictionListXchina {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Create)
    }
}

impl OptimizePlugin for FictionListXchina {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}
