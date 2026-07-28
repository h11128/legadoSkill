//! Core types for the legadoSkill repair platform (§3, §8.2, §14.6–14.7).

mod config;
mod context;
mod enums;
mod error;
mod gate;
mod identity;
mod ledger;
mod patch;
mod pattern;
mod port_types;
mod report;
mod verify;

/// Contract schema version emitted by new writers.
pub const SCHEMA_VERSION: &str = "1";

pub use config::RepairConfig;
pub use context::{HtmlCache, RepairContext, RepairContextBuilder};
pub use enums::{
    Capability, FingerprintMatchKind, GateAction, Layer, LedgerStep, MergeStrategy, Mode,
    OptimizeRisk, PatchOpKind, ReportStatus, SiteFamily, LEDGER_VERIFY_OK,
};
pub use error::{ErrorKind, PortError, TypeError};
pub use gate::{
    DiagnoseEvidence, DiagnoseResult, GateL0, GateResult, L0Hit, L1Probe, L2Probe, MigrateTarget,
};
pub use identity::{BookSource, HostKey, JsonPath, PartialBookSource, SourceKey, Url};
pub use ledger::LedgerRow;
pub use patch::{
    AdapterOutcome, MergePlan, MergeScore, NeedMoreHtml, NeedMoreHtmlKind, OptimizePlan, PatchOp,
    PatchPlan, Unrepairable, UnrepairableKind,
};
pub use pattern::{
    Fingerprint, FingerprintRule, IdentifyResult, IdentifyRunnerUp, PatternCluster,
};
pub use port_types::{CheckOpts, FetchResult, HeaderMap};
pub use report::ReportJson;
pub use verify::VerifyResult;
