//! Shared oneshot repair outcome for serial retro + reporting.

use std::process::ExitCode;

use serde_json::{json, Value};
use source_spine::{OneshotResult, REPORT_JSON_PREFIX};

#[derive(Debug, Clone)]
pub struct RepairOneOutcome {
    pub exit: ExitCode,
    pub report: Value,
}

impl RepairOneOutcome {
    pub fn err(code: u8, msg: &str) -> Self {
        Self {
            exit: ExitCode::from(code),
            report: json!({"status": "failed", "msg": msg}),
        }
    }

    pub fn from_spine(result: OneshotResult) -> Self {
        println!("{}", result.report_line);
        Self {
            exit: ExitCode::from(result.exit_code as u8),
            report: parse_report_line(&result.report_line),
        }
    }

    pub fn skipped(url: &str, message: &str) -> Self {
        let report = json!({
            "url": url,
            "status": "skipped",
            "message": message,
            "notes": [],
        });
        let line = format!(
            "{REPORT_JSON_PREFIX}{}",
            serde_json::to_string(&report).unwrap_or_default()
        );
        println!("{line}");
        Self {
            exit: ExitCode::SUCCESS,
            report,
        }
    }
}

pub fn parse_report_line(line: &str) -> Value {
    line.strip_prefix(REPORT_JSON_PREFIX)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({"raw": line}))
}
