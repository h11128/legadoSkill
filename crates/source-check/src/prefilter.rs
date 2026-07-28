//! Batch L0→L2 classify — parity with `repair_prefilter.filter_urls`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};
use source_gate::{classify_one, load_rules, ClassifyOpts, SkipRule};
use source_types::PortError;

#[derive(Debug, Clone, Default)]
pub struct PrefilterSummary {
    pub total: usize,
    pub verify_urls: Vec<String>,
    pub skip: Vec<Value>,
    pub disable: Vec<Value>,
    pub video: Vec<Value>,
    pub hunt: Vec<Value>,
    pub results: Vec<Value>,
}

fn gate_to_row(url: &str, g: &source_types::GateResult) -> Value {
    json!({
        "url": url,
        "action": g.action.as_str(),
        "reason": g.reason,
        "verify": g.verify,
    })
}

/// Classify URLs (parallel threads). Uses full L1/L2 when `source_gate` l2 feature is on.
pub fn filter_urls(
    urls: &[String],
    rules_path: &Path,
    concurrency: usize,
    l2_timeout: f64,
) -> Result<PrefilterSummary, PortError> {
    let rules: Arc<Vec<SkipRule>> = Arc::new(if rules_path.is_file() {
        load_rules(rules_path).map_err(|e| PortError::Permanent(e.to_string()))?
    } else {
        Vec::new()
    });
    let opts = ClassifyOpts {
        tcp_timeout_s: 1.5,
        l2_timeout_s: l2_timeout,
    };
    let rows: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let workers = concurrency.max(1).min(urls.len().max(1));
    let chunk = urls.len().div_ceil(workers);
    let mut handles = Vec::new();
    for part in urls.chunks(chunk.max(1)) {
        let part: Vec<String> = part.to_vec();
        let rules = Arc::clone(&rules);
        let rows = Arc::clone(&rows);
        handles.push(thread::spawn(move || {
            for url in part {
                let g = classify_one(&url, rules.as_ref(), &opts);
                rows.lock()
                    .expect("prefilter rows")
                    .push(gate_to_row(&url, &g));
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let mut results = rows.lock().expect("prefilter rows").clone();
    results.sort_by(|a, b| {
        a.get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("url").and_then(|v| v.as_str()).unwrap_or(""))
    });
    let mut summary = PrefilterSummary {
        total: results.len(),
        results,
        ..Default::default()
    };
    for row in &summary.results {
        let url = row
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let action = row.get("action").and_then(|v| v.as_str()).unwrap_or("skip");
        if row.get("verify").and_then(|v| v.as_bool()) == Some(true) {
            summary.verify_urls.push(url);
            continue;
        }
        match action {
            "disable" => summary.disable.push(row.clone()),
            "video" => summary.video.push(row.clone()),
            "hunt" => summary.hunt.push(row.clone()),
            _ => summary.skip.push(row.clone()),
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn l0_only_short_circuit() {
        let rules =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json");
        let urls = vec!["https://www.qidian.com/book/1".into()];
        let out = filter_urls(&urls, &rules, 2, 4.0).expect("filter");
        assert_eq!(out.verify_urls.len(), 0);
        assert!(!out.skip.is_empty() || !out.disable.is_empty());
    }
}
