use std::path::PathBuf;

use thiserror::Error;

/// Contract / schema validation failure.
#[derive(Debug, Error)]
pub enum ContractError {
    #[error("repo root not found (set LEGADO_SKILL_ROOT or run from crates/)")]
    RepoRootNotFound,

    #[error("schema file missing: {0}")]
    SchemaMissing(PathBuf),

    #[error("failed to read schema {path}: {source}")]
    SchemaIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("schema JSON parse error ({path}): {source}")]
    SchemaParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("schema compile error ({name}): {message}")]
    SchemaCompile { name: String, message: String },

    #[error("contract `{name}` invalid: {message}")]
    Invalid {
        name: &'static str,
        message: String,
    },
}

impl ContractError {
    pub(crate) fn invalid(name: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            name,
            message: message.into(),
        }
    }
}
