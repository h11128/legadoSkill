//! Gate classify — prefer full L0→L1→L2 when `source_gate` exports it.

use source_gate::{classify_one_l0, load_rules};
use std::path::PathBuf;
use std::process::ExitCode;

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

fn default_rules_path() -> PathBuf {
    let candidates = [
        PathBuf::from("config/verify_skip_rules.json"),
        PathBuf::from("../config/verify_skip_rules.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json"),
    ];
    for p in candidates {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json")
}

pub struct GateArgs {
    pub url: String,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
}

pub fn run_gate(args: GateArgs) -> ExitCode {
    let path = args.rules.unwrap_or_else(default_rules_path);
    let rules = match load_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gate: load rules {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let result = if args.l0_only {
        classify_one_l0(&args.url, &rules)
    } else {
        #[cfg(feature = "gate_full")]
        {
            let opts = ClassifyOpts {
                tcp_timeout_s: args.tcp_timeout,
                l2_timeout_s: args.l2_timeout,
            };
            classify_one(&args.url, &rules, &opts)
        }
        #[cfg(not(feature = "gate_full"))]
        {
            let _ = (args.tcp_timeout, args.l2_timeout);
            classify_one_l0(&args.url, &rules)
        }
    };

    match serde_json::to_string(&result) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("gate: serialize: {e}");
            ExitCode::FAILURE
        }
    }
}
