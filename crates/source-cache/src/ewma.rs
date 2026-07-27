//! EWMA gap updates — formula locked to Python `repair_cache.py`.

use serde::{Deserialize, Serialize};

pub const DEFAULT_GAP_S: f64 = 3.0;
pub const EWMA_ALPHA: f64 = 0.3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStat {
    pub ewma_gap_s: f64,
    #[serde(default)]
    pub hits: u64,
    #[serde(default)]
    pub ok: u64,
    #[serde(default)]
    pub fail: u64,
    #[serde(default)]
    pub rate_limits: u64,
    #[serde(default)]
    pub last_rate_limit_at: Option<f64>,
    #[serde(default)]
    pub last_duration_ms: Option<u64>,
    #[serde(default)]
    pub last_at: Option<f64>,
}

impl Default for HostStat {
    fn default() -> Self {
        Self {
            ewma_gap_s: DEFAULT_GAP_S,
            hits: 0,
            ok: 0,
            fail: 0,
            rate_limits: 0,
            last_rate_limit_at: None,
            last_duration_ms: None,
            last_at: None,
        }
    }
}

fn ewma(prev: f64, sample: f64) -> f64 {
    prev * (1.0 - EWMA_ALPHA) + sample * EWMA_ALPHA
}

/// `note_rate_limit` pure update (no IO).
pub fn apply_rate_limit(row: &mut HostStat, suggested_gap_s: f64, now: f64) {
    let prev = if row.ewma_gap_s == 0.0 {
        DEFAULT_GAP_S
    } else {
        row.ewma_gap_s
    };
    row.ewma_gap_s = ewma(prev, suggested_gap_s);
    row.rate_limits = row.rate_limits.saturating_add(1);
    row.last_rate_limit_at = Some(now);
}

/// `note_verify` pure update on success/fail (no IO).
pub fn apply_verify_success(
    row: &mut HostStat,
    success: bool,
    duration_ms: u64,
    used_cooldown_s: f64,
    now: f64,
) {
    row.hits = row.hits.saturating_add(1);
    if success {
        row.ok = row.ok.saturating_add(1);
        let prev = if row.ewma_gap_s == 0.0 {
            DEFAULT_GAP_S
        } else {
            row.ewma_gap_s
        };
        let target = used_cooldown_s.clamp(DEFAULT_GAP_S, 30.0);
        row.ewma_gap_s = ewma(prev, target);
    } else {
        row.fail = row.fail.saturating_add(1);
    }
    row.last_duration_ms = Some(duration_ms);
    row.last_at = Some(now);
}

/// `cooldown_for` pure.
pub fn cooldown_seconds(ewma_gap_s: f64, concurrent_rate: Option<&str>) -> f64 {
    let gap = if ewma_gap_s == 0.0 {
        DEFAULT_GAP_S
    } else {
        ewma_gap_s
    };
    let mut cr = 0.0;
    if let Some(s) = concurrent_rate {
        if let Ok(ms) = s.parse::<u64>() {
            cr = (ms as f64) / 1000.0;
        }
    }
    gap.max(cr).max(DEFAULT_GAP_S)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_moves_toward_20() {
        let mut row = HostStat::default();
        apply_rate_limit(&mut row, 20.0, 1.0);
        // 3*(0.7) + 20*(0.3) = 2.1 + 6 = 8.1
        assert!((row.ewma_gap_s - 8.1).abs() < 1e-9);
        assert_eq!(row.rate_limits, 1);
    }

    #[test]
    fn verify_success_decays() {
        let mut row = HostStat {
            ewma_gap_s: 20.0,
            ..HostStat::default()
        };
        apply_verify_success(&mut row, true, 100, 3.0, 2.0);
        // target clamp 3; 20*0.7 + 3*0.3 = 14 + 0.9 = 14.9
        assert!((row.ewma_gap_s - 14.9).abs() < 1e-9);
        assert_eq!(row.ok, 1);
    }

    #[test]
    fn cooldown_respects_concurrent_rate() {
        assert_eq!(cooldown_seconds(3.0, Some("5000")), 5.0);
        assert_eq!(cooldown_seconds(3.0, Some("abc")), 3.0);
    }
}
