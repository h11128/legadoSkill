//! Gate early-exit / diagnose-skip / channel-busy helpers for oneshot.

use source_ports::{Clock, LedgerPort};
use source_types::{
    Capability, DiagnoseResult, ErrorKind, GateAction, GateResult, Layer, LedgerRow, LedgerStep,
    MigrateTarget, PortError, RepairContext, ReportJson, ReportStatus,
};

use crate::error::SpineError;
use crate::oneshot::OneshotResult;
use crate::report_emit::emit_report_json;

pub(crate) fn channel_busy_result(
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
    let gate = ctx
        .gate
        .clone()
        .unwrap_or_else(|| GateResult::new(report.url.clone(), GateAction::Skip, "channel_busy"));
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: ErrorKind::ChannelBusy.exit_code(),
        apply: None,
        gate,
    })
}

pub(crate) fn skipped_gate(
    ctx: &RepairContext,
    gate: GateResult,
) -> Result<OneshotResult, SpineError> {
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
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: 0,
        apply: None,
        gate,
    })
}

pub(crate) fn migrated_gate(
    ctx: &RepairContext,
    gate: GateResult,
) -> Result<OneshotResult, SpineError> {
    let migrate_to = gate.migrate_to.as_ref().map(|m| match m {
        MigrateTarget::Url(u) => u.as_str().to_string(),
        MigrateTarget::Host(h) => h.as_str().to_string(),
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

pub(crate) fn diagnose_skip_result(
    ctx: &RepairContext,
    gate: GateResult,
    diag: &DiagnoseResult,
) -> Result<OneshotResult, SpineError> {
    let msg = diag
        .fail_msg
        .clone()
        .unwrap_or_else(|| format!("diagnose layer={}", layer_str(diag.layer)));
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        gate.url.clone(),
        ReportStatus::Skipped,
        msg,
    );
    report.family = Some(ctx.family.clone());
    report.layer = Some(diag.layer);
    let report_line = emit_report_json(&report)?;
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: 0,
        apply: None,
        gate,
    })
}

/// Diagnose said layer=ok: verify only (no adapter). Claim fixed only on device success.
pub(crate) use crate::oneshot_ok::diagnose_ok_verify;

pub(crate) fn unrepairable_result(
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

pub(crate) fn need_more_html_result(
    ctx: &RepairContext,
    gate: GateResult,
    why: impl Into<String>,
) -> Result<OneshotResult, SpineError> {
    let why = why.into();
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        gate.url.clone(),
        ReportStatus::Failed,
        format!("need_more_html: {why}"),
    );
    report.family = Some(ctx.family.clone());
    let report_line = emit_report_json(&report)?;
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: ErrorKind::Transient.exit_code(),
        apply: None,
        gate,
    })
}

pub(crate) fn ledger_gate<L: LedgerPort, K: Clock>(
    ledger: &L,
    clock: &K,
    ctx: &RepairContext,
    gate: &GateResult,
) -> Result<(), SpineError> {
    let ts = clock.now_utc().to_rfc3339();
    let mut row = LedgerRow::new(ts, gate.url.clone(), LedgerStep::Gate, gate.action.as_str());
    row.note = Some(gate.reason.clone());
    row.capability = Some(ctx.capability);
    row.family = Some(ctx.family.clone());
    row.report_status = Some(match gate.action {
        GateAction::Skip | GateAction::Video | GateAction::Hunt => ReportStatus::Skipped,
        GateAction::Disable => ReportStatus::Disabled,
        GateAction::Migrate => ReportStatus::Migrated,
        GateAction::Verify => ReportStatus::Fixed,
    });
    if gate.action == GateAction::Verify {
        row.report_status = None;
    }
    ledger.append(&row)?;
    Ok(())
}

fn layer_str(layer: Layer) -> &'static str {
    match layer {
        Layer::Search => "search",
        Layer::Toc => "toc",
        Layer::Content => "content",
        Layer::Explore => "explore",
        Layer::FileDownload => "file_download",
        Layer::Ok => "ok",
        Layer::Skip => "skip",
    }
}
