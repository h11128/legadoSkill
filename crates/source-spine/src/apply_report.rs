//! Apply outcome helpers (dry-run / fail / validate reports).

use source_types::{
    BookSource, ErrorKind, PatchPlan, ReportJson, ReportStatus, VerifyResult,
};

use crate::apply_ops::ops_summary;
use crate::outcome::ApplyOutcome;
use source_types::RepairContext;
use crate::error::SpineError;
use crate::report_emit::emit_report_json;

pub(crate) fn short_circuit_fixed(
    ctx: &RepairContext,
    plan: &PatchPlan,
    id_key: &str,
) -> Result<ApplyOutcome, SpineError> {
    let url = plan.source_url.clone();
    let vr = VerifyResult::new(url.clone(), true, "idempotent short-circuit", ctx.mode);
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        url,
        ReportStatus::Fixed,
        "idempotent: already verified for this ops key",
    );
    report.family = Some(plan.family.clone());
    report.verify = Some(vr.clone());
    let report_line = emit_report_json(&report)?;
    Ok(ApplyOutcome {
        idempotency_key: id_key.to_string(),
        before: ctx.source.clone(),
        after: None,
        dry_run: false,
        saved: false,
        verify: Some(vr),
        report,
        report_line,
        exit_code: 0,
        verify_failed_after_save: false,
    })
}

pub(crate) fn dry_run_outcome(
    ctx: &RepairContext,
    plan: &PatchPlan,
    id_key: &str,
) -> Result<ApplyOutcome, SpineError> {
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        plan.source_url.clone(),
        ReportStatus::Skipped,
        "dry_run: plan validated, no save",
    );
    report.family = Some(plan.family.clone());
    report.ops_summary = Some(ops_summary(&plan.ops));
    let report_line = emit_report_json(&report)?;
    Ok(ApplyOutcome {
        idempotency_key: id_key.to_string(),
        before: ctx.source.clone(),
        after: None,
        dry_run: true,
        saved: false,
        verify: None,
        report,
        report_line,
        exit_code: 0,
        verify_failed_after_save: false,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fail_report(
    ctx: &RepairContext,
    plan: &PatchPlan,
    id_key: &str,
    before: BookSource,
    after: Option<BookSource>,
    saved: bool,
    verify: Option<VerifyResult>,
    verify_failed_after_save: bool,
    message: impl Into<String>,
    exit_code: i32,
) -> Result<ApplyOutcome, SpineError> {
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        plan.source_url.clone(),
        ReportStatus::Failed,
        message,
    );
    report.family = Some(plan.family.clone());
    report.ops_summary = Some(ops_summary(&plan.ops));
    report.verify = verify.clone();
    let report_line = emit_report_json(&report)?;
    Ok(ApplyOutcome {
        idempotency_key: id_key.to_string(),
        before,
        after,
        dry_run: false,
        saved,
        verify,
        report,
        report_line,
        exit_code,
        verify_failed_after_save,
    })
}

/// Map validate failure to a failed REPORT without saving.
pub fn report_validate_failure(
    ctx: &RepairContext,
    url: source_types::Url,
    message: impl Into<String>,
) -> Result<ApplyOutcome, SpineError> {
    let mut report = ReportJson::new(
        ctx.capability,
        ctx.mode,
        url,
        ReportStatus::Failed,
        message,
    );
    report.family = Some(ctx.family.clone());
    let report_line = emit_report_json(&report)?;
    Ok(ApplyOutcome {
        idempotency_key: String::new(),
        before: ctx.source.clone(),
        after: None,
        dry_run: ctx.dry_run,
        saved: false,
        verify: None,
        report,
        report_line,
        exit_code: ErrorKind::ContractViolation.exit_code(),
        verify_failed_after_save: false,
    })
}
