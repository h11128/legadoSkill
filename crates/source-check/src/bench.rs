//! Bench10 timed wave — parity with `repair_bench10.py`.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{json, Value};
use source_types::PortError;

use crate::wave::{run_wave, WaveOpts};

pub const DEFAULT_BENCH_URLS: &[&str] = &[
    "https://www.bengben.com#🎃",
    "https://www.627txt.com##@尐哖",
    "http://www.zxcs.info/",
    "https://www.book18.org/",
    "https://www.powanjuan.cc",
    "https://www.ijjjxsw.com",
    "https://api.9yread.com/",
    "http://book.tiexue.net/",
    "http://wap.wangshugu.net#",
    "https://ifun.cool",
];

#[derive(Debug, Clone)]
pub struct BenchOpts {
    pub urls: Vec<String>,
    pub keyword: String,
    pub thread_count: u32,
    pub timeout_ms: u64,
    pub patch_workers: usize,
    pub disable_dropped: bool,
    pub out: PathBuf,
    pub rules: PathBuf,
    pub l2_timeout: f64,
}

pub fn run_bench10(opts: BenchOpts) -> Result<Value, PortError> {
    let used_defaults = opts.urls.is_empty();
    let urls = if used_defaults {
        DEFAULT_BENCH_URLS.iter().map(|s| s.to_string()).collect()
    } else {
        opts.urls
    };
    let tmp = std::env::temp_dir().join(format!("bench10_urls_{}.txt", std::process::id()));
    let body = urls.join("\n");
    std::fs::write(&tmp, &body).map_err(|e| PortError::Permanent(e.to_string()))?;

    let t0 = Instant::now();
    let wave_out = std::env::temp_dir().join(format!("bench10_wave_{}.json", std::process::id()));
    let wave_report = run_wave(WaveOpts {
        urls_file: tmp.clone(),
        keyword: opts.keyword,
        thread_count: opts.thread_count,
        patch_workers: opts.patch_workers,
        timeout_ms: opts.timeout_ms,
        check_discovery: false,
        disable_dropped: opts.disable_dropped,
        out: wave_out.clone(),
        rules: opts.rules,
        l2_timeout: opts.l2_timeout,
    })?;
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(&wave_out);

    let mut report = wave_report;
    if let Some(obj) = report.as_object_mut() {
        obj.insert("bench10".into(), json!(true));
        obj.insert("n_urls".into(), json!(urls.len()));
        obj.insert("total_s".into(), json!(t0.elapsed().as_secs_f64()));
        obj.insert("default_urls".into(), json!(used_defaults));
    }
    if let Some(parent) = opts.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| PortError::Permanent(format!("mkdir: {e}")))?;
    }
    std::fs::write(
        &opts.out,
        serde_json::to_string_pretty(&report).map_err(|e| PortError::Permanent(e.to_string()))?,
    )
    .map_err(|e| PortError::Permanent(format!("write: {e}")))?;
    Ok(report)
}
