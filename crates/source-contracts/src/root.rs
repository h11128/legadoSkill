use std::env;
use std::path::{Path, PathBuf};

use crate::error::ContractError;

const MARKER: &str = "config/repair_contracts";

/// Resolve legadoSkill repo root.
///
/// Order: `LEGADO_SKILL_ROOT` env, then walk up from `CARGO_MANIFEST_DIR`,
/// then walk up from the process current directory.
pub fn repo_root() -> Result<PathBuf, ContractError> {
    if let Ok(raw) = env::var("LEGADO_SKILL_ROOT") {
        let p = PathBuf::from(raw.trim());
        if is_repo_root(&p) {
            return Ok(p);
        }
    }

    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(found) = walk_up(&start) {
        return Ok(found);
    }

    if let Ok(cwd) = env::current_dir() {
        if let Some(found) = walk_up(&cwd) {
            return Ok(found);
        }
    }

    Err(ContractError::RepoRootNotFound)
}

fn is_repo_root(path: &Path) -> bool {
    path.join(MARKER).is_dir()
}

fn walk_up(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if is_repo_root(dir) {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_repo_from_manifest() {
        let root = repo_root().expect("repo root");
        assert!(root.join(MARKER).is_dir());
    }
}
