//! Diagnose layer=ok → device verify only (no adapter patch).

use source_ports::{Clock, LedgerPort, VerifyPort};
use source_types::{
    CheckOpts, ErrorKind, GateResult, Layer, LedgerRow, LedgerStep, RepairContext, ReportJson,
    ReportStatus, VerifyResult, LEDGER_VERIFY_OK,
};

use crate::error::SpineError;
use crate::oneshot::OneshotResult;
use crate::report_emit::emit_report_json;

/// Diagnose said layer=ok: verify only. Claim fixed only on device success.
pub(crate) fn diagnose_ok_verify<V: VerifyPort, L: LedgerPort, K: Clock>(
    ctx: &RepairContext,
    gate: GateResult,
    _diag: &source_types::DiagnoseResult,
    verify: &V,
    ledger: &L,
    clock: &K,
) -> Result<OneshotResult, SpineError> {
    if ctx.dry_run || ctx.no_verify {
        let mut report = ReportJson::new(
            ctx.capability,
            ctx.mode,
            gate.url.clone(),
            ReportStatus::Skipped,
            "diagnose layer=ok; skipped verify (dry_run/no_verify)",
        );
        report.family = Some(ctx.family.clone());
        report.layer = Some(Layer::Ok);
        let report_line = emit_report_json(&report)?;
        return Ok(OneshotResult {
            report,
            report_line,
            exit_code: 0,
            apply: None,
            gate,
        });
    }

    let vr: VerifyResult =
        match verify.check(&ctx.source_key, CheckOpts::new(ctx.config.check_discovery)) {
            Ok(v) => v,
            Err(e) => {
                let mut report = ReportJson::new(
                    ctx.capability,
                    ctx.mode,
                    gate.url.clone(),
                    ReportStatus::Failed,
                    format!("diagnose layer=ok; verify error: {e}"),
                );
                report.family = Some(ctx.family.clone());
                report.layer = Some(Layer::Ok);
                let report_line = emit_report_json(&report)?;
                return Ok(OneshotResult {
                    report,
                    report_line,
                    exit_code: e.kind().exit_code(),
                    apply: None,
                    gate,
                });
            }
        };

    let ts = clock.now_utc().to_rfc3339();
    let mut row = LedgerRow::new(
        ts,
        gate.url.clone(),
        LedgerStep::Check,
        if vr.success {
            LEDGER_VERIFY_OK
        } else {
            "verify_failed"
        },
    );
    row.note = Some("diagnose_layer_ok".into());
    row.capability = Some(ctx.capability);
    row.family = Some(ctx.family.clone());
    let _ = ledger.append(&row);

    if vr.success {
        let mut report = ReportJson::new(
            ctx.capability,
            ctx.mode,
            gate.url.clone(),
            ReportStatus::Fixed,
            "diagnose layer=ok; device verify ok",
        );
        report.family = Some(ctx.family.clone());
        report.layer = Some(Layer::Ok);
        report.verify = Some(vr);
        let report_line = emit_report_json(&report)?;
        return Ok(OneshotResult {
            report,
            report_line,
            exit_code: 0,
            apply: None,
            gate,
        });
    }

    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        gate.url.clone(),
        ReportStatus::Failed,
        format!("diagnose layer=ok but check failed: {}", vr.message),
    );
    report.family = Some(ctx.family.clone());
    report.layer = Some(Layer::Ok);
    report.verify = Some(vr);
    let report_line = emit_report_json(&report)?;
    Ok(OneshotResult {
        report,
        report_line,
        exit_code: ErrorKind::Permanent.exit_code(),
        apply: None,
        gate,
    })
}
