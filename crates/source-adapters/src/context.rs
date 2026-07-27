//! RepairContext — prefetched HTML only; no MCP ports (§14.4).

use std::collections::HashMap;

use source_types::{
    BookSource, DiagnoseResult, GateResult, RepairConfig, SiteFamily, SourceKey, Url,
};

#[derive(Debug, Clone)]
pub struct RepairContext {
    pub source_key: SourceKey,
    pub source: BookSource,
    pub gate: Option<GateResult>,
    pub diagnose: Option<DiagnoseResult>,
    pub family: SiteFamily,
    /// Prefetched pages keyed by absolute URL string.
    pub html: HashMap<String, Vec<u8>>,
    pub config: RepairConfig,
    pub dry_run: bool,
}

impl RepairContext {
    pub fn new(source_key: SourceKey, source: BookSource, family: SiteFamily) -> Self {
        Self {
            source_key,
            source,
            gate: None,
            diagnose: None,
            family,
            html: HashMap::new(),
            config: RepairConfig::default(),
            dry_run: false,
        }
    }

    pub fn with_html(mut self, url: impl AsRef<str>, body: impl Into<Vec<u8>>) -> Self {
        self.html.insert(url.as_ref().to_string(), body.into());
        self
    }

    /// UTF-8 lossy concat of all prefetched bodies (home/search).
    pub fn html_text(&self) -> String {
        let mut parts: Vec<String> = self
            .html
            .values()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect();
        parts.sort();
        parts.join("\n")
    }

    pub fn primary_url(&self) -> Result<Url, source_types::TypeError> {
        self.source_key.to_url()
    }

    pub fn base_url(&self) -> String {
        self.source_key.as_str().trim_end_matches('/').to_string()
    }
}
