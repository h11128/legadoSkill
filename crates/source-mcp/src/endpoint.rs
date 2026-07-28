//! Load `mcp_url` + `token` from `config/mcp_defaults.json`.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use source_types::PortError;

use crate::root::repo_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEndpoint {
    pub mcp_url: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct DefaultsFile {
    #[serde(default)]
    mcp_url: String,
    #[serde(default)]
    token: Option<String>,
}

impl McpEndpoint {
    pub fn new(mcp_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            mcp_url: mcp_url.into(),
            token: token.into(),
        }
    }

    /// Read shared SOT at `<repo>/config/mcp_defaults.json`.
    pub fn load_defaults() -> Result<Self, PortError> {
        let path = repo_root()?.join("config/mcp_defaults.json");
        Self::load_path(&path)
    }

    pub fn load_path(path: &Path) -> Result<Self, PortError> {
        let raw = fs::read_to_string(path)
            .map_err(|e| PortError::Permanent(format!("read {}: {e}", path.display())))?;
        let data: DefaultsFile = serde_json::from_str(&raw)
            .map_err(|e| PortError::ContractViolation(format!("mcp_defaults.json: {e}")))?;
        let token = data
            .token
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "1234".into());
        if data.mcp_url.trim().is_empty() {
            return Err(PortError::Permanent("mcp_url empty in defaults".into()));
        }
        Ok(Self::new(data.mcp_url.trim(), token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_repo_defaults() {
        let ep = McpEndpoint::load_defaults().expect("defaults");
        assert!(ep.mcp_url.starts_with("http"));
        assert!(!ep.token.is_empty());
    }
}
