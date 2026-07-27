//! Domain hunt CLI from seeds JSON.

use std::path::PathBuf;
use std::process::ExitCode;

use source_hunt::{hunt_candidates, HuntSeeds};

pub struct HuntArgs {
    pub url: String,
    pub seeds: Option<PathBuf>,
}

fn default_seeds() -> PathBuf {
    let candidates = [
        PathBuf::from("config/domain_hunt_seeds.json"),
        PathBuf::from("../config/domain_hunt_seeds.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/domain_hunt_seeds.json"),
    ];
    for p in candidates {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/domain_hunt_seeds.json")
}

pub fn run_hunt(args: HuntArgs) -> ExitCode {
    let path = args.seeds.unwrap_or_else(default_seeds);
    let seeds = match HuntSeeds::load_path(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hunt: load seeds: {e}");
            return ExitCode::from(4);
        }
    };
    let cands = hunt_candidates(&seeds, &args.url);
    let out = serde_json::json!({
        "schema_version": "1",
        "url": args.url,
        "candidates": cands,
        "status": if cands.is_empty() { "empty" } else { "hunted" },
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    ExitCode::SUCCESS
}
