use serde_json::json;
use source_cache::{apply_rate_limit, cooldown_seconds, HostStat};
use std::process::ExitCode;

pub fn run_ewma(prev: f64, suggested: f64) -> ExitCode {
    let mut row = HostStat {
        ewma_gap_s: prev,
        ..HostStat::default()
    };
    apply_rate_limit(&mut row, suggested, 0.0);
    println!(
        "{}",
        json!({
            "ewma_gap_s": row.ewma_gap_s,
            "cooldown": cooldown_seconds(row.ewma_gap_s, None),
        })
    );
    ExitCode::SUCCESS
}
