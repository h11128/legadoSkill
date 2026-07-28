//! Supporting types for port traits (§14.2) — kept in types so ports stay thin.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::identity::Url;

/// HTTP request headers for HTML fetch (string map; no `http` crate).
pub type HeaderMap = HashMap<String, String>;

/// Options for device verify. `check_discovery` defaults to false.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CheckOpts {
    pub check_discovery: bool,
}

impl CheckOpts {
    pub fn new(check_discovery: bool) -> Self {
        Self { check_discovery }
    }
}

/// Result of a PC HTML fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FetchResult {
    pub status: u16,
    pub final_url: Url,
    pub body: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl FetchResult {
    pub fn new(status: u16, final_url: Url, body: Vec<u8>) -> Self {
        Self {
            status,
            final_url,
            body,
            content_type: None,
            latency_ms: None,
        }
    }
}
