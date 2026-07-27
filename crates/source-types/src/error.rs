//! Error kinds (§14.6) and port / type errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Process / REPORT classification (§14.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Transient,
    Permanent,
    ContractViolation,
    ChannelBusy,
}

impl ErrorKind {
    /// Suggested oneshot process exit code from the architecture matrix.
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Transient => 2,
            Self::Permanent => 3,
            Self::ContractViolation => 4,
            Self::ChannelBusy => 5,
        }
    }
}

/// Errors returned by hexagonal ports (§14.2).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortError {
    #[error("transient: {0}")]
    Transient(String),
    #[error("permanent: {0}")]
    Permanent(String),
    #[error("contract violation: {0}")]
    ContractViolation(String),
    #[error("channel busy: {0}")]
    ChannelBusy(String),
}

impl PortError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Transient(_) => ErrorKind::Transient,
            Self::Permanent(_) => ErrorKind::Permanent,
            Self::ContractViolation(_) => ErrorKind::ContractViolation,
            Self::ChannelBusy(_) => ErrorKind::ChannelBusy,
        }
    }
}

/// Construction / parse errors for domain newtypes.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TypeError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
}
