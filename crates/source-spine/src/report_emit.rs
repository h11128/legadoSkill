//! REPORT_JSON stdout emission (§8.2) with contract validation.

use source_contracts::validate_report;
use source_types::ReportJson;

use crate::error::SpineError;

/// Exact prefix for streaming stdout lines.
pub const REPORT_JSON_PREFIX: &str = "REPORT_JSON:";

/// Emit `REPORT_JSON:` + compact JSON after schema validation (anti fake-fixed).
pub fn emit_report_json(report: &ReportJson) -> Result<String, SpineError> {
    let value = serde_json::to_value(report)
        .map_err(|e| SpineError::Internal(e.to_string()))?;
    validate_report(&value).map_err(|e| SpineError::Contract(e.to_string()))?;
    let body =
        serde_json::to_string(report).map_err(|e| SpineError::Internal(e.to_string()))?;
    Ok(format!("{REPORT_JSON_PREFIX}{body}"))
}

/// Prefer typed helper; falls back to `ReportJson::emit_line` shape.
pub fn emit_report_line(report: &ReportJson) -> Result<String, SpineError> {
    emit_report_json(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_types::{Capability, Mode, ReportStatus, Url};

    #[test]
    fn prefix_and_compact() {
        let r = ReportJson::new(
            Capability::Repair,
            Mode::Oneshot,
            Url::new("https://example.com/").unwrap(),
            ReportStatus::Skipped,
            "gate skip",
        );
        let line = emit_report_json(&r).unwrap();
        assert!(line.starts_with(REPORT_JSON_PREFIX));
        assert!(!line.contains('\n'));
        let json = &line[REPORT_JSON_PREFIX.len()..];
        let back: ReportJson = serde_json::from_str(json).unwrap();
        assert_eq!(back.status, ReportStatus::Skipped);
    }
}
