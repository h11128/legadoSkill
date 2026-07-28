//! Wave / harvest / serial orchestration via repair batch.

use std::path::PathBuf;
use std::process::ExitCode;

use super::repair::{run_repair, RepairArgs};

pub struct OrchestrateArgs {
    pub urls_file: PathBuf,
    pub limit: usize,
}

pub fn run_orchestrate(args: OrchestrateArgs) -> ExitCode {
    run_repair(RepairArgs {
        url: String::new(),
        urls_file: Some(args.urls_file),
        mode: "batch".into(),
        limit: args.limit,
        rules: None,
        l0_only: false,
        tcp_timeout: 1.5,
        l2_timeout: 4.0,
        dry_run: false,
        no_verify: false,
        html: None,
        prefetch: true,
        key: "我的".into(),
        skip_diagnose: false,
    })
}
