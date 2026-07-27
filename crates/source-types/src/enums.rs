//! Closed enums and SiteFamily registry ids (§3.2).

use serde::{Deserialize, Serialize};

/// Product capability verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Gate,
    Pattern,
    Identify,
    Create,
    Optimize,
    Repair,
    Merge,
    Migrate,
    Hunt,
    Check,
    Disable,
    Video,
    File,
}

/// Gate decision — maps 1:1 to `repair_prefilter.classify_one` action (§3.2).
///
/// `Video` / `Hunt` are L0 denylist extras from `verify_skip_rules.json`
/// (Python classify_one returns them; schema may still list only the four core actions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateAction {
    Verify,
    Migrate,
    Skip,
    Disable,
    Video,
    Hunt,
}

impl GateAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Migrate => "migrate",
            Self::Skip => "skip",
            Self::Disable => "disable",
            Self::Video => "video",
            Self::Hunt => "hunt",
        }
    }

    /// Parse rule / classify action; unknown → `Skip` (Python default).
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "verify" => Self::Verify,
            "migrate" => Self::Migrate,
            "disable" => Self::Disable,
            "video" => Self::Video,
            "hunt" => Self::Hunt,
            _ => Self::Skip,
        }
    }
}

/// Failure / success layer for diagnose and plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Search,
    Toc,
    Content,
    Explore,
    FileDownload,
    Ok,
    Skip,
}

/// Agent UX mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Oneshot,
    Batch,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oneshot => "oneshot",
            Self::Batch => "batch",
        }
    }
}

/// REPORT / ledger outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Fixed,
    Created,
    Optimized,
    Merged,
    Skipped,
    Failed,
    Extracted,
    Disabled,
    Migrated,
    Hunted,
}

/// Patch operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOpKind {
    Set,
    Delete,
    MigrateHost,
    MergeInto,
    DeleteSource,
    DisableSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    SameHost,
    SameFamily,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizeRisk {
    Low,
    Medium,
}

/// Ledger / observability step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerStep {
    Gate,
    Diagnose,
    Migrate,
    Hunt,
    Html,
    Probe,
    Patch,
    Apply,
    Debug,
    Check,
    Divert,
    Skip,
    Claim,
    Pattern,
    Create,
    Optimize,
    Merge,
    Disable,
}

impl LedgerStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Diagnose => "diagnose",
            Self::Migrate => "migrate",
            Self::Hunt => "hunt",
            Self::Html => "html",
            Self::Probe => "probe",
            Self::Patch => "patch",
            Self::Apply => "apply",
            Self::Debug => "debug",
            Self::Check => "check",
            Self::Divert => "divert",
            Self::Skip => "skip",
            Self::Claim => "claim",
            Self::Pattern => "pattern",
            Self::Create => "create",
            Self::Optimize => "optimize",
            Self::Merge => "merge",
            Self::Disable => "disable",
        }
    }
}

/// Fingerprint rule match kind (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FingerprintMatchKind {
    #[serde(rename = "searchUrl_regex")]
    SearchUrlRegex,
    #[serde(rename = "selector_present")]
    SelectorPresent,
    #[serde(rename = "header_charset")]
    HeaderCharset,
    #[serde(rename = "type_eq")]
    TypeEq,
    #[serde(rename = "html_regex")]
    HtmlRegex,
}

/// Open string enum backed by curated registry ids (§3.2).
///
/// Wire form is a plain string: curated id, `Unknown`, or provisional `cluster_<hash8>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SiteFamily(String);

impl SiteFamily {
    pub const JIEQI_MOBILE: &'static str = "JieqiMobile";
    pub const BOOKBENX_SEARCH81: &'static str = "BookbenxSearch81";
    pub const XUNSEARCH_PID: &'static str = "XunsearchPid";
    pub const FICTION_LIST_XCHINA: &'static str = "FictionListXchina";
    pub const EMPIRE_CMS_KEYBOARD: &'static str = "EmpireCmsKeyboard";
    pub const GONGZICP_API_WEB_VIEW: &'static str = "GongzicpApiWebView";
    pub const BIQUGE_CLASSIC: &'static str = "BiqugeClassic";
    pub const SHUBA69: &'static str = "Shuba69";
    pub const QIDIAN_JSON: &'static str = "QidianJson";
    pub const GENERIC_FORM: &'static str = "GenericForm";
    pub const UNKNOWN: &'static str = "Unknown";

    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn unknown() -> Self {
        Self::new(Self::UNKNOWN)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_unknown(&self) -> bool {
        self.0 == Self::UNKNOWN
    }

    pub fn is_provisional_cluster(&self) -> bool {
        self.0.starts_with("cluster_") && self.0.len() == "cluster_".len() + 8
    }
}

impl std::fmt::Display for SiteFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_action_serde_snake_case() {
        let json = serde_json::to_string(&GateAction::Verify).unwrap();
        assert_eq!(json, "\"verify\"");
        let back: GateAction = serde_json::from_str("\"migrate\"").unwrap();
        assert_eq!(back, GateAction::Migrate);
        let skip: GateAction = serde_json::from_str("\"skip\"").unwrap();
        assert_eq!(skip, GateAction::Skip);
        let disable: GateAction = serde_json::from_str("\"disable\"").unwrap();
        assert_eq!(disable, GateAction::Disable);
        let video: GateAction = serde_json::from_str("\"video\"").unwrap();
        assert_eq!(video, GateAction::Video);
        assert_eq!(GateAction::parse("hunt"), GateAction::Hunt);
    }
}
