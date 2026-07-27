//! Offline probe CLI (forms + rank from HTML file or string).

use std::path::PathBuf;
use std::process::ExitCode;

use source_probe::probe_search;

pub struct ProbeArgs {
    pub base_url: String,
    pub html: Option<String>,
    pub html_file: Option<PathBuf>,
    pub key: String,
}

pub fn run_probe(args: ProbeArgs) -> ExitCode {
    let html = if let Some(f) = args.html_file {
        match std::fs::read_to_string(&f) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("probe: {e}");
                return ExitCode::from(4);
            }
        }
    } else {
        args.html.unwrap_or_default()
    };
    let r = probe_search(&html, &args.base_url, &args.key);
    println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
    ExitCode::SUCCESS
}
