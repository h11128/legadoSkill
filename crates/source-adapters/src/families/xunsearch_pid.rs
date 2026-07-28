//! XunsearchPid family stub — fingerprints + HTML heuristics.

use source_types::{
    AdapterOutcome, Capability, FingerprintMatchKind, FingerprintRule, Layer, NeedMoreHtml,
    OptimizePlan, PatchOp, PatchPlan, SiteFamily, Unrepairable,
};

use source_ports::{CreatePlugin, FamilyPlugin, OptimizePlugin, RepairPlugin};
use source_types::RepairContext;

pub static XUNSEARCH_PID_RULES: &[FingerprintRule] = &[];

pub fn xunsearch_pid_rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule {
            id: "search:xunsearch_q".into(),
            weight: 2.0,
            match_kind: FingerprintMatchKind::SearchUrlRegex,
            pattern: r"search\.php\?q=".into(),
        },
        FingerprintRule {
            id: "html:xunsearch".into(),
            weight: 1.5,
            match_kind: FingerprintMatchKind::HtmlRegex,
            pattern: r"(?i)xunsearch".into(),
        },
        FingerprintRule {
            id: "html:novel_pid".into(),
            weight: 1.0,
            match_kind: FingerprintMatchKind::HtmlRegex,
            pattern: r"/novel/\d+\.html".into(),
        },
    ]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct XunsearchPid;

impl FamilyPlugin for XunsearchPid {
    fn family(&self) -> SiteFamily {
        SiteFamily::new(SiteFamily::XUNSEARCH_PID)
    }
    fn fingerprints(&self) -> &[FingerprintRule] {
        XUNSEARCH_PID_RULES
    }
}

impl XunsearchPid {
    fn propose(&self, ctx: &RepairContext, capability: Capability) -> AdapterOutcome<PatchPlan> {
        let html = ctx.html_text();
        if html.trim().is_empty() {
            return AdapterOutcome::NeedMoreHtml(NeedMoreHtml::new(
                ctx.primary_url().into_iter().collect(),
                "XunsearchPid needs search/result HTML",
            ));
        }
        let low = html.to_ascii_lowercase();
        let has_shape =
            low.contains("xunsearch") || low.contains("search.php?q=") || low.contains("/novel/");
        if !has_shape {
            return AdapterOutcome::Unrepairable(Unrepairable::new(
                "HTML lacks xunsearch/pid markers",
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
        let mut ops = vec![PatchOp::set(
            "searchUrl",
            serde_json::json!("/search.php?q={{key}}"),
        )];
        if low.contains("result") || low.contains("novel/") {
            ops.push(PatchOp::set(
                "ruleSearch.bookList",
                serde_json::json!(".result-item, .bookbox, li"),
            ));
            ops.push(
                PatchOp::set("ruleSearch.bookUrl", serde_json::json!("a@href"))
                    .with_note("pid pages often /novel/$id.html"),
            );
        }
        let mut plan = PatchPlan::new(
            capability,
            self.family(),
            url,
            ops,
            "XunsearchPid stub: search.php?q= + list heuristic",
        );
        plan.expected_layer = Some(Layer::Search);
        AdapterOutcome::Plan(plan)
    }
}

impl RepairPlugin for XunsearchPid {
    fn repair(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Repair)
    }
}

impl CreatePlugin for XunsearchPid {
    fn create(&self, ctx: &RepairContext) -> AdapterOutcome<PatchPlan> {
        self.propose(ctx, Capability::Create)
    }
}

impl OptimizePlugin for XunsearchPid {
    fn optimize(&self, _ctx: &RepairContext) -> Option<OptimizePlan> {
        None
    }
}
