//! SKILL SOT → Cursor copy sync.

use std::fs;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::paths::CloseoutPaths;

pub fn skill_fingerprint(path: &Path) -> String {
    let Ok(bytes) = fs::read(path) else {
        return String::new();
    };
    let hash = Sha256::digest(bytes);
    hex::encode(&hash[..8])
}

pub fn skill_in_sync(paths: &CloseoutPaths) -> bool {
    if !paths.skill_sot.is_file() || !paths.cursor_skill.is_file() {
        return !paths.skill_sot.is_file() && !paths.cursor_skill.is_file();
    }
    skill_fingerprint(&paths.skill_sot) == skill_fingerprint(&paths.cursor_skill)
}

pub fn sync_skill_to_cursor(paths: &CloseoutPaths) -> Result<String, String> {
    if !paths.skill_sot.is_file() {
        return Err(format!("missing SOT: {}", paths.skill_sot.display()));
    }
    if let Some(parent) = paths.cursor_skill.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&paths.skill_sot, &paths.cursor_skill).map_err(|e: io::Error| e.to_string())?;
    Ok(paths.cursor_skill.display().to_string())
}
