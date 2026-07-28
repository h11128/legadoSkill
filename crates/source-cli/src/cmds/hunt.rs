//! Domain hunt CLI from seeds JSON + optional L2 probe.

use std::path::PathBuf;
use std::process::ExitCode;

use source_gate::{classify_one, load_rules, ClassifyOpts};
use source_hunt::{hunt_candidates, HuntSeeds};
use source_types::GateAction;

pub struct HuntArgs {
    pub url: String,
    pub seeds: Option<PathBuf>,
    pub probe: bool,
    pub l2_timeout: f64,
    pub rules: Option<PathBuf>,
    pub out: Option<PathBuf>,
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

fn default_rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json")
}

fn hunt_one(
    url: &str,
    seeds: &HuntSeeds,
    rules: &[source_gate::SkipRule],
    l2_timeout: f64,
    probe: bool,
) -> serde_json::Value {
    let host = {
        let with = if url.contains("://") {
            url.to_string()
        } else {
            format!("http://{url}")
        };
        url::Url::parse(&with)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default()
    };
    let entry = seeds.lookup_host(&host);
    let cands = hunt_candidates(seeds, url);
    let shutdown = entry.map(|e| e.shutdown).unwrap_or(false);
    let confidence = entry
        .and_then(|e| e.confidence.as_deref())
        .unwrap_or("normal");
    let note = entry.and_then(|e| e.note.clone());
    let mut probes = Vec::new();
    let mut best: Option<String> = None;
    if probe {
        for c in &cands {
            let row = classify_one(
                c,
                rules,
                &ClassifyOpts {
                    tcp_timeout_s: 1.5,
                    l2_timeout_s: l2_timeout,
                },
            );
            let verify = row.action == GateAction::Verify;
            let row_json = serde_json::to_value(&row).unwrap_or(serde_json::json!({}));
            if verify && best.is_none() {
                best = Some(c.clone());
            }
            probes.push(row_json);
        }
    }
    let same = best.as_ref().is_some_and(|b| {
        b.split('#')
            .next()
            .unwrap_or(b)
            .trim_end_matches('/')
            .replace("https://", "http://")
            == url
                .split('#')
                .next()
                .unwrap_or(url)
                .trim_end_matches('/')
                .replace("https://", "http://")
    });
    let action = if shutdown {
        "no_mirror"
    } else if best.is_some() && !same && confidence != "low" {
        "migrate"
    } else if best.is_some() && !same {
        "weak_candidate"
    } else if best.is_some() {
        "original_alive"
    } else if probe {
        "none_alive"
    } else {
        "candidates_only"
    };
    serde_json::json!({
        "schema_version": "1",
        "url": url,
        "host": host,
        "note": note,
        "shutdown": shutdown,
        "confidence": confidence,
        "candidates": cands,
        "best_candidate": best,
        "probes": probes,
        "action": action,
        "status": if cands.is_empty() { "empty" } else { "hunted" },
    })
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
    let rules_path = args.rules.unwrap_or_else(default_rules);
    let rules = match load_rules(&rules_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hunt: rules: {e}");
            return ExitCode::from(4);
        }
    };
    let out = hunt_one(&args.url, &seeds, &rules, args.l2_timeout, args.probe);
    if let Some(path) = args.out {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).unwrap_or_default(),
        );
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    ExitCode::SUCCESS
}
