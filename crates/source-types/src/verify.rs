//! VerifyResult (§3.5).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::enums::Mode;
use crate::identity::Url;
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyResult {
    pub schema_version: String,
    pub url: Url,
    pub success: bool,
    pub message: String,
    pub mode: Mode,
    /// Default false for repair scoring.
    pub check_discovery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_check: Option<Value>,
}

impl VerifyResult {
    pub fn new(url: Url, success: bool, message: impl Into<String>, mode: Mode) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            url,
            success,
            message: message.into(),
            mode,
            check_discovery: false,
            duration_ms: None,
            raw_check: None,
        }
    }
}
