//! Wave / harvest / serial / bench / deep-wave / search-wave / goal15 orchestration.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use serde_json::{json, Value};
use source_check::{
    default_rules_path, load_urls_file, run_bench10, run_deep_wave, run_harvest, run_search_wave,
    run_wave, BenchOpts, DeepWaveOpts, HarvestOpts, SearchWaveOpts, WaveOpts,
};
use source_closeout::{append_retro, CloseoutPaths, RetroAppendOpts};

use super::oneshot_live::repair_one_outcome;
use super::repair::{run_repair, RepairArgs};

pub struct WaveArgs {
    pub urls_file: PathBuf,
    pub keyword: String,
    pub thread_count: u32,
    pub patch_workers: usize,
    pub timeout_ms: u64,
    pub check_discovery: bool,
    pub disable_dropped: bool,
    pub out: PathBuf,
    pub rules: Option<PathBuf>,
    pub l2_timeout: f64,
}

pub struct HarvestArgs {
    pub fails: PathBuf,
    pub limit: usize,
    pub keyword: String,
    pub thread_count: u32,
    pub timeout_ms: u64,
    pub out: PathBuf,
}

pub struct SerialArgs {
    pub urls_file: PathBuf,
    pub limit: usize,
    pub keyword: String,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    pub rules: Option<PathBuf>,
    pub rebuild_queue: bool,
    pub auto_retro: bool,
    pub out: Option<PathBuf>,
}

pub struct BenchArgs {
    pub urls_file: Option<PathBuf>,
    pub keyword: String,
    pub thread_count: u32,
    pub patch_workers: usize,
    pub timeout_ms: u64,
    pub disable_dropped: bool,
    pub out: PathBuf,
    pub rules: Option<PathBuf>,
    pub l2_timeout: f64,
}

pub struct DeepWaveArgs {
    pub urls_file: PathBuf,
    pub keyword: String,
    pub budget_s: f64,
    pub out: PathBuf,
}

pub struct SearchWaveArgs {
    pub urls_file: PathBuf,
    pub keyword: String,
    pub workers: usize,
    pub thread_count: u32,
    pub timeout_ms: u64,
    pub out: PathBuf,
}

pub struct Goal15Args {
    pub urls_file: Option<PathBuf>,
    pub keyword: String,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    pub out: PathBuf,
}

pub fn run_wave_cmd(args: WaveArgs) -> ExitCode {
    let rules = args.rules.unwrap_or_else(default_rules_path);
    match run_wave(WaveOpts {
        urls_file: args.urls_file,
        keyword: args.keyword,
        thread_count: args.thread_count,
        patch_workers: args.patch_workers,
        timeout_ms: args.timeout_ms,
        check_discovery: args.check_discovery,
        disable_dropped: args.disable_dropped,
        out: args.out,
        rules,
        l2_timeout: args.l2_timeout,
    }) {
        Ok(report) => {
            println!("{}", report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("wave: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn run_harvest_cmd(args: HarvestArgs) -> ExitCode {
    match run_harvest(HarvestOpts {
        fails: args.fails,
        limit: args.limit,
        keyword: args.keyword,
        thread_count: args.thread_count,
        timeout_ms: args.timeout_ms,
        out: args.out,
    }) {
        Ok(report) => {
            println!("{}", report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("harvest: {e}");
            if e.to_string().contains("no urls") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

pub fn run_bench_cmd(args: BenchArgs) -> ExitCode {
    let urls = args
        .urls_file
        .as_ref()
        .and_then(|p| load_urls_file(p).ok())
        .unwrap_or_default();
    let rules = args.rules.unwrap_or_else(default_rules_path);
    match run_bench10(BenchOpts {
        urls,
        keyword: args.keyword,
        thread_count: args.thread_count,
        patch_workers: args.patch_workers,
        timeout_ms: args.timeout_ms,
        disable_dropped: args.disable_dropped,
        out: args.out,
        rules,
        l2_timeout: args.l2_timeout,
    }) {
        Ok(report) => {
            println!("{}", report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("bench: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn run_deep_wave_cmd(args: DeepWaveArgs) -> ExitCode {
    match run_deep_wave(DeepWaveOpts {
        urls_file: args.urls_file,
        keyword: args.keyword,
        budget_s: args.budget_s,
        out: args.out,
    }) {
        Ok(report) => {
            println!("{}", report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("deep-wave: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn run_search_wave_cmd(args: SearchWaveArgs) -> ExitCode {
    match run_search_wave(SearchWaveOpts {
        urls_file: args.urls_file,
        keyword: args.keyword,
        workers: args.workers,
        thread_count: args.thread_count,
        timeout_ms: args.timeout_ms,
        out: args.out,
    }) {
        Ok(report) => {
            println!("{}", report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("search-wave: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn run_goal15_cmd(args: Goal15Args) -> ExitCode {
    let queue = args.urls_file.or_else(|| {
        let p = PathBuf::from("temp/full_fix/goal15_queue.json");
        if p.is_file() {
            Some(p)
        } else {
            None
        }
    });
    let code = run_repair(RepairArgs {
        url: String::new(),
        urls_file: queue,
        mode: "batch".into(),
        limit: 15,
        rules: args.rules,
        l0_only: args.l0_only,
        tcp_timeout: args.tcp_timeout,
        l2_timeout: args.l2_timeout,
        dry_run: false,
        no_verify: false,
        html: None,
        prefetch: true,
        key: args.keyword.clone(),
        skip_diagnose: false,
    });
    if let Some(out) = args.out.parent() {
        let _ = std::fs::create_dir_all(out);
    }
    let _ = std::fs::write(
        &args.out,
        json!({"goal15": true, "exit": if code == ExitCode::SUCCESS { 0 } else { 1 }}).to_string(),
    );
    code
}

fn serial_status(report: &Value) -> String {
    match report.get("status").and_then(|v| v.as_str()) {
        Some("fixed") => "fixed".into(),
        Some("skipped") => "skip".into(),
        Some("failed") => "fail".into(),
        other => other.unwrap_or("fail").to_string(),
    }
}

fn serial_trap_harness(report: &Value, waste_s: f64, status: &str) -> (String, String, String) {
    let notes_join = report
        .get("notes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let mut trap = String::new();
    let mut harness = String::new();
    let mut script_fix = String::new();
    if notes_join.contains("js_api") {
        trap = "js_search_api".into();
        script_fix = "source-probe/js_engine".into();
    }
    if waste_s > 120.0 {
        harness = "over_budget_>2min".into();
    } else if status == "fail" && waste_s > 60.0 {
        harness = "fail_after_long_probe".into();
    } else if status == "skip" && notes_join.contains("l2") {
        harness = "ok_failfast_l2".into();
        if script_fix.is_empty() {
            script_fix = "source-check/prefilter".into();
        }
    }
    (trap, harness, script_fix)
}

fn append_serial_retro(
    paths: &CloseoutPaths,
    url: &str,
    name: &str,
    report: &Value,
    waste_s: f64,
) {
    let status = serial_status(report);
    let msg = report
        .get("message")
        .or_else(|| report.get("msg"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(160)
        .collect::<String>();
    let (trap, harness, script_fix) = serial_trap_harness(report, waste_s, &status);
    match append_retro(
        paths,
        RetroAppendOpts {
            url: url.to_string(),
            status,
            msg,
            name: name.to_string(),
            respond_time: report.get("respondTime").and_then(|v| v.as_i64()),
            waste_s,
            trap,
            harness,
            script_fix,
            skill_fix: false,
            seal: false,
        },
    ) {
        Ok(_) => {}
        Err(errs) => {
            for e in errs {
                eprintln!("serial retro: {e}");
            }
        }
    }
}

pub fn run_serial_cmd(args: SerialArgs) -> ExitCode {
    if args.rebuild_queue {
        use source_queue::{build_rt_queue_full, default_serial_queue_path, RtBuildOpts};
        let index = PathBuf::from("temp/full_fix/phone_source_index.json");
        let out = default_serial_queue_path()
            .unwrap_or_else(|_| PathBuf::from("temp/full_fix/queues/repair_serial100_queue.json"));
        if let Ok(doc) = build_rt_queue_full(
            &index,
            &RtBuildOpts {
                max_rt_ms: 8000,
                limit: args.limit.max(1),
                enabled_only: true,
                search_tag_only: true,
                all_sources_path: None,
                ledger_path: None,
            },
        ) {
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                &out,
                serde_json::to_string_pretty(&doc).unwrap_or_default(),
            );
            eprintln!("serial: rebuilt RT queue → {}", out.display());
        }
    }
    let urls = match load_urls_file(&args.urls_file) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("serial: {e}");
            return ExitCode::from(4);
        }
    };
    let closeout = if args.auto_retro {
        CloseoutPaths::from_repo().ok()
    } else {
        None
    };
    let lim = args.limit.max(1);
    let mut last = ExitCode::SUCCESS;
    let t0_all = Instant::now();
    let mut summary = json!({
        "n_in": urls.len(),
        "limit": lim,
        "results": [],
    });
    for (idx, url) in urls.into_iter().take(lim).enumerate() {
        let t0 = Instant::now();
        let outcome = repair_one_outcome(
            &url,
            args.rules.clone(),
            args.l0_only,
            args.tcp_timeout,
            args.l2_timeout,
            false,
            false,
            None,
            true,
            &args.keyword,
            false,
        );
        let waste_s = t0.elapsed().as_secs_f64();
        let exit_code = if outcome.exit == ExitCode::SUCCESS { 0 } else { 1 };
        let name = outcome
            .report
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(paths) = closeout.as_ref() {
            append_serial_retro(paths, &url, &name, &outcome.report, waste_s);
        }
        summary["results"]
            .as_array_mut()
            .expect("results array")
            .push(json!({
                "n": idx + 1,
                "url": url,
                "exit": exit_code,
                "status": serial_status(&outcome.report),
                "waste_s": waste_s,
                "name": name,
            }));
        if outcome.exit != ExitCode::SUCCESS {
            last = outcome.exit;
        }
    }
    summary["elapsed_s"] = json!(t0_all.elapsed().as_secs_f64());
    let out_path = args
        .out
        .unwrap_or_else(|| PathBuf::from("temp/full_fix/serial_last.json"));
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &out_path,
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
    );
    println!("{}", summary);
    last
}
