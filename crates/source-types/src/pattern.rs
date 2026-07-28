//! Pattern / Identify shapes (§3.4).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::enums::{FingerprintMatchKind, SiteFamily};
use crate::identity::{PartialBookSource, Url};
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub signals: Vec<String>,
    pub structural_hash: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FingerprintRule {
    pub id: String,
    pub weight: f64,
    #[serde(rename = "match")]
    pub match_kind: FingerprintMatchKind,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternCluster {
    pub schema_version: String,
    pub family: SiteFamily,
    pub size: u32,
    pub fingerprint: Fingerprint,
    pub centroid: PartialBookSource,
    pub exemplars: Vec<Url>,
    #[serde(default)]
    pub coverage: HashMap<String, f64>,
    pub extracted_at: String,
    /// Template / adapter version (§14.13); optional for schema `"1"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_version: Option<u32>,
}

impl PatternCluster {
    pub fn new(
        family: SiteFamily,
        size: u32,
        fingerprint: Fingerprint,
        centroid: PartialBookSource,
        exemplars: Vec<Url>,
        extracted_at: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            family,
            size,
            fingerprint,
            centroid,
            exemplars,
            coverage: HashMap::new(),
            extracted_at: extracted_at.into(),
            adapter_version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentifyRunnerUp {
    pub family: SiteFamily,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentifyResult {
    pub schema_version: String,
    pub url: Url,
    pub family: SiteFamily,
    pub fingerprint: Fingerprint,
    pub evidence_urls: Vec<Url>,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_up: Option<IdentifyRunnerUp>,
}

impl IdentifyResult {
    pub fn new(url: Url, family: SiteFamily, fingerprint: Fingerprint, score: f64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            url,
            family,
            fingerprint,
            evidence_urls: Vec::new(),
            score,
            runner_up: None,
        }
    }
}
