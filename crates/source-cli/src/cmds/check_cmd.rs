//! Bulk check / channel / precheck CLI.

use std::path::PathBuf;
use std::process::ExitCode;

use source_check::{
    channel_status, load_urls_file, precheck_json, precheck_urls, run_batch_check, BatchCheckOpts,
};

pub enum CheckCmd {
    Channel,
    Precheck {
        urls_file: PathBuf,
        timeout: f64,
    },
    Batch {
        urls_file: PathBuf,
        keyword: String,
        batch_size: usize,
        thread_count: u32,
        timeout: f64,
    },
    Full {
        urls_file: PathBuf,
        keyword: String,
        batch_size: usize,
        thread_count: u32,
        timeout: f64,
    },
}

fn run_batch(urls: Vec<String>, keyword: String, batch_size: usize, thread_count: u32, timeout: f64) -> ExitCode {
    match run_batch_check(BatchCheckOpts {
        urls,
        keyword,
        thread_count,
        batch_size,
        timeout_ms: (timeout * 1000.0) as u64,
        check_discovery: false,
    }) {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::json!({
                    "started": summary.started,
                    "success": summary.success,
                    "failed": summary.failed,
                    "results": summary.results,
                })
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("check batch: {e}");
            ExitCode::from(1)
        }
    }
}

pub fn run_check(cmd: CheckCmd) -> ExitCode {
    match cmd {
        CheckCmd::Channel => match channel_status() {
            Ok(v) => {
                println!("{}", v);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("check channel: {e}");
                ExitCode::from(1)
            }
        },
        CheckCmd::Precheck { urls_file, timeout } => {
            let urls = match load_urls_file(&urls_file) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(4);
                }
            };
            println!("{}", precheck_json(&urls, timeout));
            ExitCode::SUCCESS
        }
        CheckCmd::Batch {
            urls_file,
            keyword,
            batch_size,
            thread_count,
            timeout,
        } => {
            let urls = match load_urls_file(&urls_file) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(4);
                }
            };
            run_batch(urls, keyword, batch_size, thread_count, timeout)
        }
        CheckCmd::Full {
            urls_file,
            keyword,
            batch_size,
            thread_count,
            timeout,
        } => {
            let urls = match load_urls_file(&urls_file) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(4);
                }
            };
            let precheck_timeout = timeout.min(8.0);
            let alive: Vec<String> = precheck_urls(&urls, precheck_timeout)
                .into_iter()
                .filter(|r| r.ok)
                .map(|r| r.url)
                .collect();
            eprintln!(
                "check full: precheck alive {}/{} (timeout {:.1}s)",
                alive.len(),
                urls.len(),
                precheck_timeout
            );
            run_batch(alive, keyword, batch_size, thread_count, timeout)
        }
    }
}
