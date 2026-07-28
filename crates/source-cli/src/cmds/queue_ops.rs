//! `source-cli queue build|classify|why` — fail-queue / decide / why buckets.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{json, Value};
use source_queue::{
    build_fail_queue, classify_resolved_url, decide, load_items, why_report, write_fail_queue,
};

pub enum QueueOpsCmd {
    Build {
        input: PathBuf,
        out: PathBuf,
        limit: usize,
    },
    Classify {
        fail_msg: Option<String>,
        url: Option<String>,
        html: Option<String>,
        html_file: Option<PathBuf>,
    },
    Why {
        input: PathBuf,
        out: Option<PathBuf>,
    },
}

pub fn run_queue_ops(cmd: QueueOpsCmd) -> ExitCode {
    match run_inner(cmd) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("queue: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_inner(cmd: QueueOpsCmd) -> Result<Value, String> {
    match cmd {
        QueueOpsCmd::Build { input, out, limit } => {
            let items = load_items(&input).map_err(|e| e.to_string())?;
            let enriched = build_fail_queue(&items, limit);
            write_fail_queue(&out, &enriched).map_err(|e| e.to_string())?;
            Ok(json!({
                "path": out.display().to_string(),
                "n": enriched.len(),
                "preview": enriched.iter().take(10).cloned().collect::<Vec<_>>(),
            }))
        }
        QueueOpsCmd::Classify {
            fail_msg,
            url,
            html,
            html_file,
        } => {
            if let Some(msg) = fail_msg {
                let d = decide(&msg, None);
                return Ok(json!({"mode": "decide", "fail_msg": msg, "decision": d}));
            }
            let Some(url) = url else {
                return Err("need --fail-msg and/or --url".into());
            };
            let html_s = if let Some(h) = html {
                Some(h)
            } else if let Some(p) = html_file {
                Some(std::fs::read_to_string(p).map_err(|e| e.to_string())?)
            } else {
                None
            };
            let v = classify_resolved_url(&url, html_s.as_deref());
            Ok(json!({"mode": "url_kind", "result": v}))
        }
        QueueOpsCmd::Why { input, out } => {
            let raw: Value =
                serde_json::from_str(&std::fs::read_to_string(&input).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let rows = if let Some(arr) = raw.as_array() {
                arr.clone()
            } else if let Some(arr) = raw.get("rows").and_then(|v| v.as_array()) {
                arr.clone()
            } else {
                return Err("why input must be JSON array or {rows:[…]}".into());
            };
            let report = why_report(rows);
            if let Some(path) = out {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(
                    &path,
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                Ok(json!({
                    "out": path.display().to_string(),
                    "buckets": report.get("buckets"),
                    "n": report.get("rows").and_then(|r| r.as_array()).map(|a| a.len()),
                }))
            } else {
                Ok(report)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn build_writes_sorted_queue() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("fail.jsonl");
        std::fs::write(
            &input,
            r#"{"url":"https://b/","message":"未知"}
{"url":"https://a/","fail_msg":"目录失效"}
"#,
        )
        .unwrap();
        let out = dir.path().join("q.json");
        let v = run_inner(QueueOpsCmd::Build {
            input,
            out: out.clone(),
            limit: 50,
        })
        .unwrap();
        assert_eq!(v["n"], 2);
        let written: Value = serde_json::from_str(&std::fs::read_to_string(out).unwrap()).unwrap();
        assert_eq!(written[0]["url"], "https://a/");
    }

    #[test]
    fn classify_decide_and_why() {
        let d = run_inner(QueueOpsCmd::Classify {
            fail_msg: Some("搜索失效".into()),
            url: None,
            html: None,
            html_file: None,
        })
        .unwrap();
        assert_eq!(d["decision"]["layer"], "search");
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("why.json");
        std::fs::write(&input, r#"[{"http_err":"HTTP 404"},{"debug_books":0}]"#).unwrap();
        let out = dir.path().join("report.json");
        let r = run_inner(QueueOpsCmd::Why {
            input,
            out: Some(out.clone()),
        })
        .unwrap();
        assert!(out.is_file());
        assert!(r["buckets"]["dead_404"].as_u64().unwrap_or(0) >= 1);
    }
}
