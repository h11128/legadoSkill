//! RepairContext builder — no MCP / port handles (§14.4).

use std::collections::HashMap;

use source_types::{
    BookSource, Capability, DiagnoseResult, GateResult, Mode, RepairConfig, SiteFamily, SourceKey,
    Url,
};

/// Prefetched HTML keyed by absolute URL (bytes only — no live fetch in ctx).
pub type HtmlCache = HashMap<Url, Vec<u8>>;

/// Immutable-ish repair inputs for plugins and apply (§14.4).
#[derive(Debug, Clone)]
pub struct RepairContext {
    pub source_key: SourceKey,
    pub source: BookSource,
    pub gate: Option<GateResult>,
    pub diagnose: Option<DiagnoseResult>,
    pub family: SiteFamily,
    pub html: HtmlCache,
    pub config: RepairConfig,
    pub dry_run: bool,
    /// Skip device verify after save (CLI `--no-verify`).
    pub no_verify: bool,
    /// Default off — matches Python caution (§14.5).
    pub rollback_on_verify_fail: bool,
    pub mode: Mode,
    pub capability: Capability,
}

impl RepairContext {
    pub fn builder(source_key: SourceKey, source: BookSource) -> RepairContextBuilder {
        RepairContextBuilder::new(source_key, source)
    }
}

#[derive(Debug, Clone)]
pub struct RepairContextBuilder {
    source_key: SourceKey,
    source: BookSource,
    gate: Option<GateResult>,
    diagnose: Option<DiagnoseResult>,
    family: SiteFamily,
    html: HtmlCache,
    config: RepairConfig,
    dry_run: bool,
    no_verify: bool,
    rollback_on_verify_fail: bool,
    mode: Mode,
    capability: Capability,
}

impl RepairContextBuilder {
    pub fn new(source_key: SourceKey, source: BookSource) -> Self {
        Self {
            source_key,
            source,
            gate: None,
            diagnose: None,
            family: SiteFamily::unknown(),
            html: HtmlCache::new(),
            config: RepairConfig::default(),
            dry_run: false,
            no_verify: false,
            rollback_on_verify_fail: false,
            mode: Mode::Oneshot,
            capability: Capability::Repair,
        }
    }

    pub fn gate(mut self, gate: GateResult) -> Self {
        self.gate = Some(gate);
        self
    }

    pub fn diagnose(mut self, diagnose: DiagnoseResult) -> Self {
        self.diagnose = Some(diagnose);
        self
    }

    pub fn family(mut self, family: SiteFamily) -> Self {
        self.family = family;
        self
    }

    pub fn html(mut self, html: HtmlCache) -> Self {
        self.html = html;
        self
    }

    pub fn insert_html(mut self, url: Url, body: Vec<u8>) -> Self {
        self.html.insert(url, body);
        self
    }

    pub fn config(mut self, config: RepairConfig) -> Self {
        self.config = config;
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn no_verify(mut self, no_verify: bool) -> Self {
        self.no_verify = no_verify;
        self
    }

    pub fn rollback_on_verify_fail(mut self, on: bool) -> Self {
        self.rollback_on_verify_fail = on;
        self
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    pub fn capability(mut self, capability: Capability) -> Self {
        self.capability = capability;
        self
    }

    pub fn build(self) -> RepairContext {
        RepairContext {
            source_key: self.source_key,
            source: self.source,
            gate: self.gate,
            diagnose: self.diagnose,
            family: self.family,
            html: self.html,
            config: self.config,
            dry_run: self.dry_run,
            no_verify: self.no_verify,
            rollback_on_verify_fail: self.rollback_on_verify_fail,
            mode: self.mode,
            capability: self.capability,
        }
    }
}
