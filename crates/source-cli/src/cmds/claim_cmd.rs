//! Session claim / index — parity with `repair_claim.py`.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use source_closeout::{append_index, assert_fixed_allowed, load_check_json};

pub enum ClaimCmd {
    Validate {
        check_json: PathBuf,
    },
    AppendIndex {
        index: PathBuf,
        status: String,
        url: String,
        name: Option<String>,
        evidence: Option<String>,
        agent: Option<String>,
        root_cause: Option<String>,
    },
}

pub fn run_claim(cmd: ClaimCmd) -> ExitCode {
    match cmd {
        ClaimCmd::Validate { check_json } => match load_check_json(&check_json) {
            Ok(v) => match assert_fixed_allowed(Some(&v)) {
                Ok(()) => {
                    println!("{}", json!({"ok": true, "check": v}));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("claim validate: {e}");
                    ExitCode::from(1)
                }
            },
            Err(e) => {
                eprintln!("claim validate: {e}");
                ExitCode::from(4)
            }
        },
        ClaimCmd::AppendIndex {
            index,
            status,
            url,
            name,
            evidence,
            agent,
            root_cause,
        } => {
            let entry = json!({
                "status": status,
                "url": url,
                "name": name,
                "evidence": evidence,
                "agent": agent.unwrap_or_else(|| "source-cli".into()),
                "root_cause": root_cause,
            });
            match append_index(&index, &entry) {
                Ok(data) => {
                    println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("claim index: {e}");
                    ExitCode::from(1)
                }
            }
        }
    }
}
