//! LedgerRow (§3.5).

use serde::{Deserialize, Serialize};

use crate::enums::{Capability, Layer, LedgerStep, ReportStatus, SiteFamily};
use crate::identity::Url;
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRow {
    pub schema_version: String,
    /// ISO-8601 UTC.
    pub ts: String,
    pub url: Url,
    pub step: LedgerStep,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waste: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<SiteFamily>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_status: Option<ReportStatus>,
}

impl LedgerRow {
    pub fn new(
        ts: impl Into<String>,
        url: Url,
        step: LedgerStep,
        result: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            ts: ts.into(),
            url,
            step,
            result: result.into(),
            note: None,
            waste: None,
            capability: None,
            family: None,
            layer: None,
            report_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_row_serde_roundtrip() {
        let row = LedgerRow {
            note: Some("l2 wall".into()),
            capability: Some(Capability::Repair),
            family: Some(SiteFamily::unknown()),
            layer: Some(Layer::Search),
            report_status: Some(ReportStatus::Skipped),
            ..LedgerRow::new(
                "2026-07-26T12:00:00Z",
                Url::new("https://example.com/").unwrap(),
                LedgerStep::Gate,
                "skip",
            )
        };
        let json = serde_json::to_string(&row).unwrap();
        let back: LedgerRow = serde_json::from_str(&json).unwrap();
        assert_eq!(back, row);
        assert_eq!(back.schema_version, "1");
        assert_eq!(back.step, LedgerStep::Gate);
    }
}
