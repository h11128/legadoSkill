//! Repo-relative paths for close-out artifacts.

use std::path::{Path, PathBuf};

use source_mcp::repo_root;

#[derive(Debug, Clone)]
pub struct CloseoutPaths {
    pub root: PathBuf,
    pub skill_sot: PathBuf,
    pub cursor_skill: PathBuf,
    pub ledger: PathBuf,
    pub retro: PathBuf,
}

impl CloseoutPaths {
    pub fn from_repo() -> Result<Self, String> {
        let root = repo_root().map_err(|e| e.to_string())?;
        Ok(Self::under(&root))
    }

    pub fn under(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.clone());
        Self {
            skill_sot: root.join("skills/legado-book-source-repair/SKILL.md"),
            cursor_skill: home.join(".cursor/skills/legado-book-source-repair/SKILL.md"),
            ledger: root.join("temp/full_fix/repair_session_ledger.jsonl"),
            retro: root.join("temp/full_fix/repair_serial_retro.jsonl"),
            root,
        }
    }
}

pub fn norm_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}
