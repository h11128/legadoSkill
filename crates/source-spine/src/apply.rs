//! ApplyService — validate → dry_run → snapshot → apply → save → verify → ledger (§14.5).

use source_contracts::validate_patch;
use source_ports::{Clock, LedgerPort, SourceRepository, VerifyPort};
use source_types::{
    CheckOpts, ErrorKind, LedgerRow, LedgerStep, PatchPlan, PortError, ReportJson, ReportStatus,
    LEDGER_VERIFY_OK,
};

use crate::apply_ops::{apply_ops_to_source, ops_summary};
use crate::apply_report::{
    dry_run_outcome, fail_report, report_validate_failure, short_circuit_fixed,
};
use crate::error::SpineError;
use crate::idempotency::idempotency_key;
use crate::outcome::{ApplyOutcome, IdempotencyStore};
use crate::report_emit::emit_report_json;
use source_types::RepairContext;

/// Apply a validated patch plan through ports only (DIP).
pub struct ApplyService;

impl ApplyService {
    /// Full §14.5 pipeline. `idem` is optional short-circuit store.
    pub fn apply<R, V, L, C>(
        ctx: &RepairContext,
        plan: &PatchPlan,
        repo: &R,
        verify: &V,
        ledger: &L,
        clock: &C,
        idem: Option<&mut dyn IdempotencyStore>,
    ) -> Result<ApplyOutcome, SpineError>
    where
        R: SourceRepository,
        V: VerifyPort,
        L: LedgerPort,
        C: Clock,
    {
        if let Err(e) = Self::validate_patch_plan(plan) {
            return report_validate_failure(ctx, plan.source_url.clone(), e.to_string());
        }

        let id_key = idempotency_key(&ctx.source_key, &plan.ops)?;
        if let Some(store) = idem.as_ref() {
            if store.last_verify_ok(&id_key) {
                return short_circuit_fixed(ctx, plan, &id_key);
            }
        }

        if ctx.dry_run {
            return dry_run_outcome(ctx, plan, &id_key);
        }

        let before = repo.get(&ctx.source_key)?;
        let after = apply_ops_to_source(&before, &plan.ops)?;

        if let Err(e) = repo.save(&after) {
            let mut out = fail_report(
                ctx,
                plan,
                &id_key,
                before,
                None,
                false,
                None,
                false,
                format!("save failed: {e}"),
                e.kind().exit_code(),
            )?;
            if let Err(le) = ledger_note(
                ledger,
                clock,
                ctx,
                LedgerStep::Apply,
                &out.report.message,
                None,
            ) {
                out.report.message = format!("{}; ledger: {le}", out.report.message);
                out.report_line = emit_report_json(&out.report)?;
            }
            return Ok(out);
        }

        if ctx.no_verify {
            let mut out = fail_report(
                ctx,
                plan,
                &id_key,
                before,
                Some(after),
                true,
                None,
                false,
                "saved without verify (no_verify); not claiming fixed",
                ErrorKind::ContractViolation.exit_code(),
            )?;
            if let Err(le) = ledger_note(
                ledger,
                clock,
                ctx,
                LedgerStep::Apply,
                &out.report.message,
                None,
            ) {
                out.report.message = format!("{}; ledger: {le}", out.report.message);
                out.report_line = emit_report_json(&out.report)?;
            }
            return Ok(out);
        }

        let vr = match verify.check(&ctx.source_key, CheckOpts::new(ctx.config.check_discovery)) {
            Ok(vr) => vr,
            Err(e) => {
                if ctx.rollback_on_verify_fail {
                    let _ = repo.save(&before);
                }
                let mut out = fail_report(
                    ctx,
                    plan,
                    &id_key,
                    before,
                    Some(after),
                    true,
                    None,
                    true,
                    format!("verify error: {e}"),
                    e.kind().exit_code(),
                )?;
                if let Err(le) = ledger_note(
                    ledger,
                    clock,
                    ctx,
                    LedgerStep::Check,
                    &out.report.message,
                    Some("verify_failed_after_save"),
                ) {
                    out.report.message = format!("{}; ledger: {le}", out.report.message);
                    out.report_line = emit_report_json(&out.report)?;
                }
                return Ok(out);
            }
        };

        if !vr.success {
            if ctx.rollback_on_verify_fail {
                let _ = repo.save(&before);
            }
            let msg = vr.message.clone();
            let mut out = fail_report(
                ctx,
                plan,
                &id_key,
                before,
                Some(after),
                true,
                Some(vr),
                true,
                msg,
                ErrorKind::Permanent.exit_code(),
            )?;
            if let Err(le) = ledger_note(
                ledger,
                clock,
                ctx,
                LedgerStep::Check,
                "verify_failed_after_save",
                Some("verify_failed_after_save"),
            ) {
                out.report.message = format!("{}; ledger: {le}", out.report.message);
                out.report_line = emit_report_json(&out.report)?;
            }
            return Ok(out);
        }

        if let Some(store) = idem {
            store.remember_ok(&id_key);
        }
        ledger_note(
            ledger,
            clock,
            ctx,
            LedgerStep::Check,
            LEDGER_VERIFY_OK,
            None,
        )?;

        let mut report = ReportJson::new(
            ctx.capability,
            ctx.mode,
            plan.source_url.clone(),
            ReportStatus::Fixed,
            "patched; device verify ok",
        );
        report.family = Some(plan.family.clone());
        report.layer = plan.expected_layer;
        report.ops_summary = Some(ops_summary(&plan.ops));
        report.verify = Some(vr.clone());
        let report_line = emit_report_json(&report)?;
        Ok(ApplyOutcome {
            idempotency_key: id_key,
            before,
            after: Some(after),
            dry_run: false,
            saved: true,
            verify: Some(vr),
            report,
            report_line,
            exit_code: 0,
            verify_failed_after_save: false,
        })
    }

    pub fn validate_patch_plan(plan: &PatchPlan) -> Result<(), SpineError> {
        if plan.ops.is_empty() {
            return Err(SpineError::Contract("patch_plan ops empty".into()));
        }
        let value = serde_json::to_value(plan).map_err(|e| SpineError::Internal(e.to_string()))?;
        validate_patch(&value)?;
        Ok(())
    }
}

fn ledger_note<L: LedgerPort, C: Clock>(
    ledger: &L,
    clock: &C,
    ctx: &RepairContext,
    step: LedgerStep,
    result: &str,
    note: Option<&str>,
) -> Result<(), SpineError> {
    let ts = clock.now_utc().to_rfc3339();
    let url = ctx
        .source_key
        .to_url()
        .map_err(|e| PortError::ContractViolation(e.to_string()))?;
    let mut row = LedgerRow::new(ts, url, step, result);
    row.note = note.map(|s| s.to_string());
    row.capability = Some(ctx.capability);
    row.family = Some(ctx.family.clone());
    ledger.append(&row)?;
    Ok(())
}
