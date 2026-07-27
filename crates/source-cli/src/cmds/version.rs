//! Print crate / schema versions for operators and parity shims.

use serde_json::json;
use source_types::SCHEMA_VERSION;
use std::process::ExitCode;

pub fn run_version() -> ExitCode {
    println!(
        "{}",
        json!({
            "cli": env!("CARGO_PKG_NAME"),
            "cli_version": env!("CARGO_PKG_VERSION"),
            "schema_version": SCHEMA_VERSION,
            "crates": {
                "source_types": env!("CARGO_PKG_VERSION"),
                "source_gate": env!("CARGO_PKG_VERSION"),
                "source_video": env!("CARGO_PKG_VERSION"),
                "source_cache": env!("CARGO_PKG_VERSION"),
                "source_probe": env!("CARGO_PKG_VERSION"),
            }
        })
    );
    ExitCode::SUCCESS
}
