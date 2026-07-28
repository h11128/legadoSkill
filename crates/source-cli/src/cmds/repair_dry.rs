//! Dry-run oneshot: gate → AdapterRegistry → spine (mem ports). No MCP writes.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::{TimeZone, Utc};
use serde_json::json;
use source_adapters::{AdapterRegistry, RegistryRepairPlugin};
use source_gate::{classify_one_l0, load_rules};
use source_spine::fakes::{FixedClock, IdleChannel, MemLedger, MemRepo, MemVerify};
use source_spine::{run_repair_oneshot, DiagnoseInput, GateInput, PlanOrPlugin, RepairPorts};
use source_types::{BookSource, SourceKey, Url};

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

pub struct RepairDryArgs {
    pub url: String,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    /// Optional HTML file to seed identify/adapters (UTF-8).
    pub html: Option<PathBuf>,
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

pub fn run_repair_dry(args: RepairDryArgs) -> ExitCode {
    let path = args.rules.unwrap_or_else(default_rules_path);
    let rules = match load_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("repair-dry: load rules: {e}");
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

    let key = SourceKey::new(args.url.trim());
    let source = BookSource::new(json!({
        "bookSourceUrl": key.as_str(),
        "bookSourceName": "cli-dry",
        "bookSourceType": 0,
        "enabled": true,
    }));

    let mut builder = source_spine::RepairContext::builder(key, source)
        .gate(gate.clone())
        .dry_run(true)
        .no_verify(true);
    if let Some(html_path) = &args.html {
        match std::fs::read(html_path) {
            Ok(body) => match Url::new(args.url.trim()) {
                Ok(u) => builder = builder.insert_html(u, body),
                Err(e) => {
                    eprintln!("repair-dry: bad url for html inject: {e}");
                    return ExitCode::from(4);
                }
            },
            Err(e) => {
                eprintln!("repair-dry: read html {}: {e}", html_path.display());
                return ExitCode::from(4);
            }
        }
    }
    let ctx = builder.build();

    let repo = MemRepo::with_source(ctx.source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock(Utc.timestamp_opt(1_700_000_000, 0).unwrap());
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let reg = AdapterRegistry::with_seed_families();
    let plugin = RegistryRepairPlugin(&reg);
    let result = match run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&plugin),
        GateInput::Injected(gate),
        Some(&reg),
        DiagnoseInput::None,
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("repair-dry: spine: {e}");
            return ExitCode::from(2);
        }
    };

    println!("{}", result.report_line);
    ExitCode::from(result.exit_code as u8)
}
