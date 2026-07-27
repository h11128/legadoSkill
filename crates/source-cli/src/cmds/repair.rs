//! Live oneshot / batch repair via MCP ports + AdapterRegistry.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;

use super::oneshot_live::repair_one_url;

pub struct RepairArgs {
    pub url: String,
    pub urls_file: Option<PathBuf>,
    pub mode: String,
    pub limit: usize,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    pub dry_run: bool,
    pub no_verify: bool,
    pub html: Option<PathBuf>,
    pub prefetch: bool,
    pub key: String,
    pub skip_diagnose: bool,
}

fn load_urls_file(path: &PathBuf) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        out.push(t.to_string());
    }
    Ok(out)
}

fn collect_urls(args: &RepairArgs) -> Result<Vec<String>, String> {
    let mut urls = Vec::new();
    if let Some(path) = &args.urls_file {
        urls.extend(load_urls_file(path)?);
    }
    let u = args.url.trim();
    if !u.is_empty() {
        urls.push(u.to_string());
    }
    if urls.is_empty() {
        return Err("need --url or --urls-file".into());
    }
    if args.mode == "oneshot" {
        urls.truncate(1);
    } else {
        let lim = args.limit.max(1);
        urls.truncate(lim);
    }
    Ok(urls)
}

pub fn run_repair(args: RepairArgs) -> ExitCode {
    let urls = match collect_urls(&args) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("repair: {e}");
            return ExitCode::from(2);
        }
    };
    println!(
        "{}",
        json!({
            "mode": args.mode,
            "n": urls.len(),
        })
    );
    let mut last = ExitCode::SUCCESS;
    let mut results = Vec::new();
    for url in &urls {
        let code = repair_one_url(
            url,
            args.rules.clone(),
            args.l0_only,
            args.tcp_timeout,
            args.l2_timeout,
            args.dry_run,
            args.no_verify,
            args.html.clone(),
            args.prefetch,
            &args.key,
            args.skip_diagnose,
        );
        let ok = code == ExitCode::SUCCESS;
        results.push(json!({"url": url, "ok": ok}));
        last = code;
        if args.mode == "oneshot" {
            break;
        }
    }
    if args.mode == "batch" {
        println!("{}", json!({"mode": "batch", "results": results}));
    }
    last
}
