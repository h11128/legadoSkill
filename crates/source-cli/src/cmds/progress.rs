//! Progress status/next — L2-gated candidates from phone index JSON.

use std::path::PathBuf;
use std::process::ExitCode;

use source_gate::{classify_one_l0, load_rules, SkipRule};
use serde_json::{json, Value};

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

pub struct ProgressArgs {
    pub cmd: String, // status | next
    pub index: Option<PathBuf>,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
}

fn default_index() -> PathBuf {
    let c = [
        PathBuf::from("temp/full_fix/phone_source_index.json"),
        PathBuf::from("../temp/full_fix/phone_source_index.json"),
    ];
    for p in c {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("temp/full_fix/phone_source_index.json")
}

fn default_rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json")
}

fn load_index(path: &PathBuf) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn candidate_urls(index: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if let Some(by) = index.get("by_url").and_then(|v| v.as_object()) {
        for (url, meta) in by {
            let g = meta.get("group").and_then(|v| v.as_str()).unwrap_or("");
            if !g.contains("搜索失效") {
                continue;
            }
            if meta.get("enabled") == Some(&json!(false)) {
                continue;
            }
            if !url.starts_with("http") {
                continue;
            }
            out.push((url.clone(), meta.clone()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn gate_one(url: &str, rules: &[SkipRule], l0_only: bool) -> Value {
    let g = if l0_only {
        classify_one_l0(url, rules)
    } else {
        #[cfg(feature = "gate_full")]
        {
            classify_one(
                url,
                rules,
                &ClassifyOpts {
                    tcp_timeout_s: 1.5,
                    l2_timeout_s: 4.0,
                },
            )
        }
        #[cfg(not(feature = "gate_full"))]
        {
            classify_one_l0(url, rules)
        }
    };
    serde_json::to_value(&g).unwrap_or(json!({}))
}

pub fn run_progress(args: ProgressArgs) -> ExitCode {
    let index_path = args.index.unwrap_or_else(default_index);
    let index = match load_index(&index_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("progress: index: {e}");
            return ExitCode::from(4);
        }
    };
    let rules_path = args.rules.unwrap_or_else(default_rules);
    let rules = match load_rules(&rules_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("progress: rules: {e}");
            return ExitCode::from(4);
        }
    };
    let cands = candidate_urls(&index);
    if args.cmd == "status" {
        println!(
            "{}",
            json!({
                "index": index_path.to_string_lossy(),
                "search_fail_candidates": cands.len(),
                "total": index.get("total").cloned().unwrap_or(json!(null)),
            })
        );
        return ExitCode::SUCCESS;
    }
    // next: first that gate action == verify
    for (url, meta) in cands.iter().take(40) {
        let g = gate_one(url, &rules, args.l0_only);
        let action = g.get("action").and_then(|a| a.as_str()).unwrap_or("");
        if action == "verify" {
            println!(
                "{}",
                json!({
                    "next": {
                        "url": url,
                        "name": meta.get("name"),
                        "group": meta.get("group"),
                        "l2_gate": g,
                        "status": "candidate",
                    }
                })
            );
            return ExitCode::SUCCESS;
        }
    }
    println!("{}", json!({"next": null, "hint": "no verify-pass candidate in first 40"}));
    ExitCode::SUCCESS
}
