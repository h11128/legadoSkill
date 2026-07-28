//! Rust parity harness — replaces parity_selftest.py core suites.

use std::process::{Command, ExitCode};

use source_mcp::repo_root;

pub struct ParityArgs {
    pub suite: Option<String>,
}

pub fn run_parity(args: ParityArgs) -> ExitCode {
    let suites: Vec<String> = if let Some(s) = args.suite {
        vec![s]
    } else {
        vec!["rust-cli".into(), "contracts".into(), "inventory".into()]
    };

    let mut results = Vec::new();
    for suite in &suites {
        let ok = match suite.as_str() {
            "rust-cli" | "contracts" => cargo_test_workspace(),
            "inventory" => check_no_py_scripts(),
            "perf" => check_perf_baseline_exists(),
            other => {
                eprintln!("parity: unknown suite {other}");
                false
            }
        };
        results.push((suite, ok));
    }
    let passed = results.iter().filter(|(_, ok)| *ok).count();
    let summary = serde_json::json!({
        "ok": passed == results.len(),
        "suites_run": results.len(),
        "suites_passed": passed,
        "results": results.iter().map(|(n, ok)| serde_json::json!({"suite": n, "ok": ok})).collect::<Vec<_>>(),
    });
    println!("SUMMARY {}", summary);
    if summary.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn workspace_dir() -> Result<std::path::PathBuf, source_types::PortError> {
    let root = repo_root()?;
    let nested = root.join("crates");
    if nested.join("Cargo.toml").is_file() {
        Ok(nested)
    } else if root.join("Cargo.toml").is_file() {
        Ok(root)
    } else {
        Err(source_types::PortError::Permanent(
            "Cargo workspace not found (expected crates/Cargo.toml)".into(),
        ))
    }
}

fn cargo_test_workspace() -> bool {
    let dir = match workspace_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("parity: workspace: {e}");
            return false;
        }
    };
    Command::new("cargo")
        .args(["test", "--workspace", "--quiet"])
        .current_dir(dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn check_no_py_scripts() -> bool {
    let root = match repo_root() {
        Ok(r) => r,
        Err(_) => return false,
    };
    let scripts = root.join("scripts");
    if !scripts.is_dir() {
        return true;
    }
    std::fs::read_dir(&scripts)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .all(|e| e.path().extension().is_none_or(|x| x != "py"))
        })
        .unwrap_or(false)
}

fn check_perf_baseline_exists() -> bool {
    repo_root()
        .map(|r| r.join("docs/parity/PERF_BASELINE.json").is_file())
        .unwrap_or(false)
}
