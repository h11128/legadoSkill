//! RepairConfig — single config surface (§14.7).

use serde::{Deserialize, Serialize};

/// Defaults → `config/repair_config.json` → env → CLI (load order owned by CLI later).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairConfig {
    pub identify_min_score: f64,
    pub identify_margin: f64,
    pub cluster_min_size: u32,
    pub ewma_alpha: f64,
    pub default_gap_s: f64,
    pub l1_timeout_s: f64,
    pub l2_timeout_s: f64,
    pub gate_concurrency: u32,
    pub check_discovery: bool,
    pub soft_budget_s: u64,
    pub hard_budget_s: u64,
    pub dual_write_sqlite: bool,
}

impl Default for RepairConfig {
    fn default() -> Self {
        Self {
            identify_min_score: 2.0,
            identify_margin: 0.5,
            cluster_min_size: 3,
            ewma_alpha: 0.3,
            default_gap_s: 3.0,
            l1_timeout_s: 1.5,
            l2_timeout_s: 4.0,
            gate_concurrency: 32,
            check_discovery: false,
            soft_budget_s: 240,
            hard_budget_s: 300,
            dual_write_sqlite: true,
        }
    }
}

impl RepairConfig {
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        let mut cfg = Self::default();
        let overlay: RepairConfigOverlay = serde_json::from_str(s)?;
        overlay.apply(&mut cfg);
        Ok(cfg)
    }
}

/// Partial overlay so missing keys keep defaults.
#[derive(Debug, Default, Deserialize)]
struct RepairConfigOverlay {
    identify_min_score: Option<f64>,
    identify_margin: Option<f64>,
    cluster_min_size: Option<u32>,
    ewma_alpha: Option<f64>,
    default_gap_s: Option<f64>,
    l1_timeout_s: Option<f64>,
    l2_timeout_s: Option<f64>,
    gate_concurrency: Option<u32>,
    check_discovery: Option<bool>,
    soft_budget_s: Option<u64>,
    hard_budget_s: Option<u64>,
    dual_write_sqlite: Option<bool>,
}

impl RepairConfigOverlay {
    fn apply(self, cfg: &mut RepairConfig) {
        if let Some(v) = self.identify_min_score {
            cfg.identify_min_score = v;
        }
        if let Some(v) = self.identify_margin {
            cfg.identify_margin = v;
        }
        if let Some(v) = self.cluster_min_size {
            cfg.cluster_min_size = v;
        }
        if let Some(v) = self.ewma_alpha {
            cfg.ewma_alpha = v;
        }
        if let Some(v) = self.default_gap_s {
            cfg.default_gap_s = v;
        }
        if let Some(v) = self.l1_timeout_s {
            cfg.l1_timeout_s = v;
        }
        if let Some(v) = self.l2_timeout_s {
            cfg.l2_timeout_s = v;
        }
        if let Some(v) = self.gate_concurrency {
            cfg.gate_concurrency = v;
        }
        if let Some(v) = self.check_discovery {
            cfg.check_discovery = v;
        }
        if let Some(v) = self.soft_budget_s {
            cfg.soft_budget_s = v;
        }
        if let Some(v) = self.hard_budget_s {
            cfg.hard_budget_s = v;
        }
        if let Some(v) = self.dual_write_sqlite {
            cfg.dual_write_sqlite = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_config_defaults() {
        let cfg = RepairConfig::default();
        assert_eq!(cfg.identify_min_score, 2.0);
        assert_eq!(cfg.identify_margin, 0.5);
        assert_eq!(cfg.cluster_min_size, 3);
        assert_eq!(cfg.ewma_alpha, 0.3);
        assert_eq!(cfg.default_gap_s, 3.0);
        assert_eq!(cfg.l1_timeout_s, 1.5);
        assert_eq!(cfg.l2_timeout_s, 4.0);
        assert_eq!(cfg.gate_concurrency, 32);
        assert!(!cfg.check_discovery);
        assert_eq!(cfg.soft_budget_s, 240);
        assert_eq!(cfg.hard_budget_s, 300);
        assert!(cfg.dual_write_sqlite);
    }

    #[test]
    fn repair_config_partial_json_keeps_defaults() {
        let cfg = RepairConfig::from_json_str(r#"{"gate_concurrency":8}"#).unwrap();
        assert_eq!(cfg.gate_concurrency, 8);
        assert_eq!(cfg.identify_min_score, 2.0);
        assert!(!cfg.check_discovery);
    }
}
