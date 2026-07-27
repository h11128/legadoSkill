//! Spine errors mapped to §14.6 REPORT status / oneshot exit codes.

use source_contracts::ContractError;
use source_patch::ApplyError;
use source_types::{ErrorKind, PortError, ReportStatus};
use thiserror::Error;

/// Orchestration / apply failure.
#[derive(Debug, Error)]
pub enum SpineError {
    #[error("contract: {0}")]
    Contract(String),
    #[error("patch apply: {0}")]
    Patch(#[from] ApplyError),
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("unrepairable: {0}")]
    Unrepairable(String),
    #[error("need more html: {0}")]
    NeedMoreHtml(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl From<ContractError> for SpineError {
    fn from(e: ContractError) -> Self {
        Self::Contract(e.to_string())
    }
}

impl SpineError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Contract(_) => ErrorKind::ContractViolation,
            Self::Patch(_) => ErrorKind::ContractViolation,
            Self::Port(p) => p.kind(),
            Self::Unrepairable(_) => ErrorKind::Permanent,
            Self::NeedMoreHtml(_) => ErrorKind::Transient,
            Self::Internal(_) => ErrorKind::ContractViolation,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.kind().exit_code()
    }

    pub fn report_status(&self) -> ReportStatus {
        match self.kind() {
            ErrorKind::Permanent => ReportStatus::Skipped,
            _ => ReportStatus::Failed,
        }
    }
}
