use serde_json::Value;

use crate::error::ContractError;
use crate::registry::validate_named;

/// Validate a streaming `REPORT_JSON` object.
pub fn validate_report(value: &Value) -> Result<(), ContractError> {
    validate_named("report_json", value)
}

/// Validate a gate (L0/L1/L2) result.
pub fn validate_gate(value: &Value) -> Result<(), ContractError> {
    validate_named("gate_result", value)
}

/// Validate a diagnose result.
pub fn validate_diagnose(value: &Value) -> Result<(), ContractError> {
    validate_named("diagnose_result", value)
}

/// Validate a patch plan (non-empty ops required by schema).
pub fn validate_patch(value: &Value) -> Result<(), ContractError> {
    validate_named("patch_plan", value)
}

/// Validate an optimize plan.
pub fn validate_optimize(value: &Value) -> Result<(), ContractError> {
    validate_named("optimize_plan", value)
}

/// Validate a merge plan.
pub fn validate_merge(value: &Value) -> Result<(), ContractError> {
    validate_named("merge_plan", value)
}

/// Validate a device verify result.
pub fn validate_verify(value: &Value) -> Result<(), ContractError> {
    validate_named("verify_result", value)
}

/// Validate a ledger row.
pub fn validate_ledger(value: &Value) -> Result<(), ContractError> {
    validate_named("ledger_row", value)
}

/// Validate a pattern cluster.
pub fn validate_pattern(value: &Value) -> Result<(), ContractError> {
    validate_named("pattern_cluster", value)
}

/// Validate an identify result.
pub fn validate_identify(value: &Value) -> Result<(), ContractError> {
    validate_named("identify_result", value)
}
