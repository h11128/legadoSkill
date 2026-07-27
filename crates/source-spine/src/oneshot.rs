//! Oneshot orchestration: Gate → Diagnose → Identify → propose → Apply → Verify → Ledger.

use source_ports::{
    ChannelPort, Clock, DiagnosePort, IdentifyPort, LedgerPort, RepairPlugin, SourceRepository,
    VerifyPort,
};
use source_types::{
    AdapterOutcome, GateAction, GateResult, Layer, PatchPlan, PortError, RepairContext,
};

use crate::apply::ApplyService;
use crate::error::SpineError;
use crate::oneshot_gate::{
    channel_busy_result, diagnose_ok_verify, diagnose_skip_result, ledger_gate, migrated_gate,
    need_more_html_result, skipped_gate, unrepairable_result,
};
use crate::outcome::{ApplyOutcome, IdempotencyStore};
use crate::plugin::identify_stub;
use source_types::ReportJson;

/// Injected gate result or callable classifier.
pub enum GateInput<'a> {
    Injected(GateResult),
    Fn(&'a dyn GateFn),
}

/// Callback form of gate (PC L0/L1/L2 or test stub).
pub trait GateFn {
    fn classify(&self, ctx: &RepairContext) -> Result<GateResult, PortError>;
}

impl<F> GateFn for F
where
    F: Fn(&RepairContext) -> Result<GateResult, PortError>,
{
    fn classify(&self, ctx: &RepairContext) -> Result<GateResult, PortError> {
        self(ctx)
    }
}

/// Prebuilt plan or plugin propose.
pub enum PlanOrPlugin<'a> {
    Plan(PatchPlan),
    Plugin(&'a dyn RepairPlugin),
}

/// Port bundle for oneshot (traits only — wiring stays in CLI).
pub struct RepairPorts<'a, R, V, L, C, K> {
    pub repo: &'a R,
    pub verify: &'a V,
    pub ledger: &'a L,
    pub channel: &'a C,
    pub clock: &'a K,
}

/// Result of `run_repair_oneshot`.
#[derive(Debug, Clone)]
pub struct OneshotResult {
    pub report: ReportJson,
    pub report_line: String,
    pub exit_code: i32,
    pub apply: Option<ApplyOutcome>,
    pub gate: GateResult,
}

/// Optional diagnose input: precomputed, from debug text via port, or skip.
pub enum DiagnoseInput<'a> {
    /// Already computed (tests / CLI).
    Ready(source_types::DiagnoseResult),
    /// Call port with debug log text (MCP debug_source output).
    DebugText {
        text: &'a str,
        fail_msg: Option<&'a str>,
        port: &'a dyn DiagnosePort,
    },
    /// No diagnose step.
    None,
}

/// Gate → Diagnose → Identify → propose → Apply → Verify → Ledger.
pub fn run_repair_oneshot<R, V, L, C, K>(
    mut ctx: RepairContext,
    ports: &RepairPorts<'_, R, V, L, C, K>,
    plan_or_adapter: PlanOrPlugin<'_>,
    gate_input: GateInput<'_>,
    identify: Option<&dyn IdentifyPort>,
    diagnose: DiagnoseInput<'_>,
    idem: Option<&mut dyn IdempotencyStore>,
) -> Result<OneshotResult, SpineError>
where
    R: SourceRepository,
    V: VerifyPort,
    L: LedgerPort,
    C: ChannelPort,
    K: Clock,
{
    if let Err(e) = ports.channel.assert_idle_for_repair() {
        return channel_busy_result(&ctx, e);
    }

    let gate = match gate_input {
        GateInput::Injected(g) => g,
        GateInput::Fn(f) => f.classify(&ctx)?,
    };
    ctx.gate = Some(gate.clone());
    ledger_gate(ports.ledger, ports.clock, &ctx, &gate)?;

    if matches!(
        gate.action,
        GateAction::Skip
            | GateAction::Disable
            | GateAction::Video
            | GateAction::Hunt
            | GateAction::Migrate
    ) {
        if gate.action == GateAction::Migrate {
            return migrated_gate(&ctx, gate);
        }
        return skipped_gate(&ctx, gate);
    }

    // Diagnose (optional)
    match diagnose {
        DiagnoseInput::None => {}
        DiagnoseInput::Ready(d) => {
            if d.layer == Layer::Skip {
                ctx.diagnose = Some(d.clone());
                return diagnose_skip_result(&ctx, gate, &d);
            }
            if d.layer == Layer::Ok {
                ctx.diagnose = Some(d.clone());
                return diagnose_ok_verify(
                    &ctx,
                    gate,
                    &d,
                    ports.verify,
                    ports.ledger,
                    ports.clock,
                );
            }
            ctx.diagnose = Some(d);
        }
        DiagnoseInput::DebugText {
            text,
            fail_msg,
            port,
        } => {
            let url = ctx
                .source_key
                .to_url()
                .map_err(|e| PortError::ContractViolation(e.to_string()))?;
            let d = port.diagnose(url, &ctx.source, text, fail_msg);
            if d.layer == Layer::Skip {
                ctx.diagnose = Some(d.clone());
                return diagnose_skip_result(&ctx, gate, &d);
            }
            if d.layer == Layer::Ok {
                ctx.diagnose = Some(d.clone());
                return diagnose_ok_verify(
                    &ctx,
                    gate,
                    &d,
                    ports.verify,
                    ports.ledger,
                    ports.clock,
                );
            }
            ctx.diagnose = Some(d);
        }
    }

    let identified = match identify {
        Some(port) => {
            let url = ctx
                .source_key
                .to_url()
                .map_err(|e| PortError::ContractViolation(e.to_string()))?;
            let html = ctx.html_text();
            port.identify(url, &ctx.source, &html, &ctx.config)
        }
        None => identify_stub(&ctx),
    };
    ctx.family = identified.family.clone();

    // Layer-aware: search prefers GenericForm when unknown
    if ctx.family.is_unknown() {
        if let Some(d) = &ctx.diagnose {
            if d.layer == Layer::Search {
                ctx.family = source_types::SiteFamily::new(source_types::SiteFamily::GENERIC_FORM);
            }
        }
    }

    let plan = match plan_or_adapter {
        PlanOrPlugin::Plan(p) => p,
        PlanOrPlugin::Plugin(plugin) => match plugin.repair(&ctx) {
            AdapterOutcome::Plan(p) => p,
            AdapterOutcome::Unrepairable(u) => {
                return unrepairable_result(&ctx, gate, u.reason);
            }
            AdapterOutcome::NeedMoreHtml(n) => {
                return need_more_html_result(&ctx, gate, n.why);
            }
        },
    };

    let outcome = ApplyService::apply(
        &ctx,
        &plan,
        ports.repo,
        ports.verify,
        ports.ledger,
        ports.clock,
        idem,
    )?;

    Ok(OneshotResult {
        report: outcome.report.clone(),
        report_line: outcome.report_line.clone(),
        exit_code: outcome.exit_code,
        apply: Some(outcome),
        gate,
    })
}
