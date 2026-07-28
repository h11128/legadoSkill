//! Defaults from `config/repair_db_defaults.json` (parity with `repair_db.load_cfg`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_DB_REL: &str = "temp/full_fix/repair_state.sqlite";
pub const DEFAULT_SNAPSHOT_TTL_S: f64 = 600.0;
pub const DEFAULT_PHONE_TTL_S: f64 = 3600.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairDbCfg {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_phone_ttl")]
    pub phone_index_ttl_s: f64,
    #[serde(default = "default_snapshot_ttl")]
    pub source_snapshot_ttl_s: f64,
    #[serde(default = "default_true")]
    pub dual_write_ledger: bool,
    #[serde(default = "default_true")]
    pub sync_html_meta_on_put: bool,
    #[serde(default = "default_true")]
    pub sync_host_stats_on_put: bool,
}

fn default_db_path() -> String {
    DEFAULT_DB_REL.to_string()
}
fn default_phone_ttl() -> f64 {
    DEFAULT_PHONE_TTL_S
}
fn default_snapshot_ttl() -> f64 {
    DEFAULT_SNAPSHOT_TTL_S
}
fn default_true() -> bool {
    true
}

impl Default for RepairDbCfg {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            phone_index_ttl_s: DEFAULT_PHONE_TTL_S,
            source_snapshot_ttl_s: DEFAULT_SNAPSHOT_TTL_S,
            dual_write_ledger: true,
            sync_html_meta_on_put: true,
            sync_host_stats_on_put: true,
        }
    }
}

/// Load cfg from `root/config/repair_db_defaults.json`, or defaults.
pub fn load_cfg(root: &Path) -> RepairDbCfg {
    let path = root.join("config/repair_db_defaults.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return RepairDbCfg::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Resolve absolute DB path from cfg + repo root.
pub fn db_path(root: &Path, cfg: &RepairDbCfg) -> PathBuf {
    let p = PathBuf::from(&cfg.db_path);
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

/// Merge optional JSON overrides into cfg (CLI / tests).
pub fn cfg_from_value(v: &Value) -> RepairDbCfg {
    serde_json::from_value(v.clone()).unwrap_or_default()
}
