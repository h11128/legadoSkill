//! Offline + MCP diagnose CLI.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use source_contracts::validate_diagnose;
use source_diagnose::{diagnose_from_debug, diagnose_gate_skip, gate_blocks_diagnose};
use source_gate::{classify_one_l0, load_rules};
use source_mcp::{McpClient, McpEndpoint, McpSourceRepository};
use source_ports::SourceRepository;
use source_types::{SourceKey, Url};

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

pub struct DiagnoseArgs {
    pub url: String,
    pub key: String,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    /// Skip MCP debug_source; diagnose from this debug log file.
    pub debug_file: Option<PathBuf>,
}

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

pub fn run_diagnose(args: DiagnoseArgs) -> ExitCode {
    let path = args.rules.unwrap_or_else(default_rules_path);
    let rules = match load_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("diagnose: load rules: {e}");
            return ExitCode::from(4);
        }
    };
    let gate = if args.l0_only {
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

    let url = match Url::new(args.url.trim()) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("diagnose: bad url: {e}");
            return ExitCode::from(4);
        }
    };

    if gate_blocks_diagnose(&gate) {
        let d = diagnose_gate_skip(url, gate);
        let v = serde_json::to_value(&d).unwrap_or_default();
        let _ = validate_diagnose(&v);
        println!("{}", serde_json::to_string_pretty(&d).unwrap_or_default());
        return ExitCode::from(3);
    }

    let debug_text = if let Some(p) = &args.debug_file {
        match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("diagnose: read debug file: {e}");
                return ExitCode::from(4);
            }
        }
    } else {
        let ep = match McpEndpoint::load_defaults() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("diagnose: mcp: {e}");
                return ExitCode::from(4);
            }
        };
        let client = Arc::new(McpClient::new(ep).with_client_name("source_cli_diagnose"));
        if let Err(e) = client.ensure_session() {
            eprintln!("diagnose: session: {e}");
            return ExitCode::from(2);
        }
        let repo = McpSourceRepository::new(Arc::clone(&client));
        let _ = repo.get(&SourceKey::new(args.url.trim()));
        let raw = match client.tools_call(
            "debug_source",
            serde_json::json!({"url": args.url, "key": args.key}),
        ) {
            Ok(v) => McpClient::extract_text(&v),
            Err(e) => {
                eprintln!("diagnose: debug_source: {e}");
                return ExitCode::from(2);
            }
        };
        raw
    };

    let d = diagnose_from_debug(url, &debug_text, Some(gate), None);
    let v = serde_json::to_value(&d).unwrap_or_default();
    if let Err(e) = validate_diagnose(&v) {
        eprintln!("diagnose: contract: {e}");
        return ExitCode::from(4);
    }
    println!("{}", serde_json::to_string_pretty(&d).unwrap_or_default());
    ExitCode::SUCCESS
}
