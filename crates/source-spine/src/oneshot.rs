//! Oneshot orchestration: Gate → Identify(stub) → propose → Apply → Verify → Ledger.

use source_ports::{ChannelPort, Clock, LedgerPort, SourceRepository, VerifyPort};
use source_types::{
    AdapterOutcome, Capability, ErrorKind, GateAction, GateResult, LedgerRow, LedgerStep, PatchPlan,
    PortError, ReportJson, ReportStatus,
};

use crate::apply::ApplyService;
use crate::outcome::{ApplyOutcome, IdempotencyStore};
use crate::context::RepairContext;
use crate::error::SpineError;
use crate::plugin::{identify_stub, RepairPlugin};
use crate::report_emit::emit_report_json;

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

/// Gate → Identify(stub) → propose → Apply → Verify → Ledger (§4.1 / §14.5).
pub fn run_repair_oneshot<R, V, L, C, K>(
    mut ctx: RepairContext,
    ports: &RepairPorts<'_, R, V, L, C, K>,
    plan_or_adapter: PlanOrPlugin<'_>,
    gate_input: GateInput<'_>,
    idem: Option<&mut dyn IdempotencyStore>,
) -> Result<OneshotResult, SpineError>
where
    R: SourceRepository,
    V: VerifyPort,
    L: LedgerPort,
    C: ChannelPort,
    K: Clock,
{
    // Channel busy → no MCP; REPORT failed `channel_busy` (§14.5).
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

    // Identify stub (real identify lands when source_identify is ready).
    let identified = identify_stub(&ctx);
    ctx.family = identified.family.clone();

    let plan = match plan_or_adapter {
        PlanOrPlugin::Plan(p) => p,
        PlanOrPlugin::Plugin(plugin) => match plugin.repair(&ctx) {
            AdapterOutcome::Plan(p) => p,
            AdapterOutcome::Unrepairable(u) => {
                return unrepairable_result(&ctx, gate, u.reason);
            }
            AdapterOutcome::NeedMoreHtml(n) => {
                return Err(SpineError::NeedMoreHtml(n.why));
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

fn channel_busy_result(
    ctx: &RepairContext,
    err: PortError,
) -> Result<OneshotResult, SpineError> {
    let url = ctx
        .source_key
        .to_url()
        .map_err(|e| PortError::ContractViolation(e.to_string()))?;
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        url,
        ReportStatus::Failed,
        format!("channel_busy: {err}"),
    );
    report.family = Some(ctx.family.clone());
    let report_line = emit_report_json(&report)?;
    let gate = ctx.gate.clone().unwrap_or_else(|| {
        GateResult::new(report.url.clone(), GateAction::Skip, "channel_busy")
    });
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: ErrorKind::ChannelBusy.exit_code(),
        apply: None,
        gate,
    })
}

fn skipped_gate(ctx: &RepairContext, gate: GateResult) -> Result<OneshotResult, SpineError> {
    let (status, capability) = if gate.action == GateAction::Disable {
        (ReportStatus::Disabled, Capability::Disable)
    } else {
        (ReportStatus::Skipped, ctx.capability)
    };
    let mut report = ReportJson::new(
        capability,
        ctx.mode,
        gate.url.clone(),
        status,
        format!("gate {}: {}", gate.action.as_str(), gate.reason),
    );
    report.family = Some(ctx.family.clone());
    let report_line = emit_report_json(&report)?;
    // Permanent skip → exit 0 (expected skip) per §14.6 soft reading.
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: 0,
        apply: None,
        gate,
    })
}

fn migrated_gate(ctx: &RepairContext, gate: GateResult) -> Result<OneshotResult, SpineError> {
    let migrate_to = gate
        .migrate_to
        .as_ref()
        .map(|m| match m {
            source_types::MigrateTarget::Url(u) => u.as_str().to_string(),
            source_types::MigrateTarget::Host(h) => h.as_str().to_string(),
        });
    let msg = match &migrate_to {
        Some(to) => format!("gate migrate: {} → {to}", gate.reason),
        None => format!("gate migrate: {} (missing migrate_to)", gate.reason),
    };
    let mut report = ReportJson::new(
        Capability::Migrate,
        ctx.mode,
        gate.url.clone(),
        ReportStatus::Migrated,
        msg,
    );
    report.family = Some(ctx.family.clone());
    report.migrate_to = migrate_to;
    let report_line = emit_report_json(&report)?;
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: 0,
        apply: None,
        gate,
    })
}

fn unrepairable_result(
    ctx: &RepairContext,
    gate: GateResult,
    reason: impl Into<String>,
) -> Result<OneshotResult, SpineError> {
    let reason = reason.into();
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        gate.url.clone(),
        ReportStatus::Skipped,
        reason,
    );
    report.family = Some(ctx.family.clone());
    let report_line = emit_report_json(&report)?;
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: 0,
        apply: None,
        gate,
    })
}

fn ledger_gate<L: LedgerPort, K: Clock>(
    ledger: &L,
    clock: &K,
    ctx: &RepairContext,
    gate: &GateResult,
) -> Result<(), SpineError> {
    let ts = clock.now_utc().to_rfc3339();
    let mut row = LedgerRow::new(
        ts,
        gate.url.clone(),
        LedgerStep::Gate,
        gate.action.as_str(),
    );
    row.note = Some(gate.reason.clone());
    row.capability = Some(ctx.capability);
    row.family = Some(ctx.family.clone());
    row.report_status = Some(match gate.action {
        GateAction::Skip | GateAction::Video | GateAction::Hunt => ReportStatus::Skipped,
        GateAction::Disable => ReportStatus::Disabled,
        GateAction::Migrate => ReportStatus::Migrated,
        GateAction::Verify => ReportStatus::Fixed, // placeholder until apply; overwritten later
    });
    // Avoid implying fixed at gate-verify: use None for verify-pass gate rows.
    if gate.action == GateAction::Verify {
        row.report_status = None;
    }
    ledger.append(&row)?;
    Ok(())
}
