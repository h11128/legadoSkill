//! REPORT_JSON stdout shape (§8.2).

use serde::{Deserialize, Serialize};

use crate::enums::{Capability, Layer, Mode, ReportStatus, SiteFamily};
use crate::identity::Url;
use crate::verify::VerifyResult;
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportJson {
    pub schema_version: String,
    pub capability: Capability,
    pub mode: Mode,
    pub url: Url,
    pub status: ReportStatus,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<SiteFamily>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ops_summary: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_to: Option<String>,
    /// Required when status is fixed|created|optimized|merged (contracts enforce later).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyResult>,
}

impl ReportJson {
    pub fn new(
        capability: Capability,
        mode: Mode,
        url: Url,
        status: ReportStatus,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            capability,
            mode,
            url,
            status,
            message: message.into(),
            family: None,
            layer: None,
            duration_ms: None,
            fixed_n: None,
            ops_summary: None,
            migrate_to: None,
            verify: None,
        }
    }

    /// Compact single-line emit: `REPORT_JSON:` + JSON.
    pub fn emit_line(&self) -> Result<String, serde_json::Error> {
        let body = serde_json::to_string(self)?;
        Ok(format!("REPORT_JSON:{body}"))
    }
}
