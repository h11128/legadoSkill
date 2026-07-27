//! Gate and diagnose result shapes (§3.3).

use serde::{Deserialize, Serialize};

use crate::enums::{GateAction, Layer};
use crate::identity::{HostKey, Url};
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1Probe {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L2Probe {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadish: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_migrated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_host: Option<HostKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_host: Option<HostKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L0Hit {
    pub rule_id: String,
    pub action: GateAction,
    pub reason: String,
}

/// Alias used by some call sites / older drafts.
pub type GateL0 = L0Hit;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateResult {
    pub schema_version: String,
    pub url: Url,
    /// Legacy alias: true only when `action == verify`.
    pub verify: bool,
    pub action: GateAction,
    /// Machine id, e.g. `passed_l0_l1_l2`.
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_to: Option<MigrateTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l0: Option<L0Hit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l1: Option<L1Probe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l2: Option<L2Probe>,
}

impl GateResult {
    pub fn new(url: Url, action: GateAction, reason: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            url,
            verify: action == GateAction::Verify,
            action,
            reason: reason.into(),
            migrate_to: None,
            l0: None,
            l1: None,
            l2: None,
        }
    }

    /// L0 denylist hit — `verify: false`, `l0` filled (Python classify_one L0 branch).
    pub fn l0_deny(url: Url, hit: L0Hit) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            url,
            verify: false,
            action: hit.action,
            reason: hit.reason.clone(),
            migrate_to: None,
            l0: Some(hit),
            l1: None,
            l2: None,
        }
    }

    /// L0-only pass (L1/L2 not run yet).
    pub fn passed_l0(url: Url) -> Self {
        Self::new(url, GateAction::Verify, "passed_l0")
    }
}

/// `migrate_to` may be a host key or absolute URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MigrateTarget {
    Url(Url),
    Host(HostKey),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DiagnoseEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toc_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnoseResult {
    pub schema_version: String,
    pub url: Url,
    pub layer: Layer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_msg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fake_detail: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reclassified_from: Option<Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateResult>,
    #[serde(default)]
    pub evidence: DiagnoseEvidence,
    /// Human repair tips (probe best / traps). Empty omitted in JSON.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tips: Vec<String>,
}

impl DiagnoseResult {
    pub fn new(url: Url, layer: Layer) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            url,
            layer,
            fail_msg: None,
            fake_detail: None,
            reclassified_from: None,
            gate: None,
            evidence: DiagnoseEvidence::default(),
            tips: Vec::new(),
        }
    }
}
