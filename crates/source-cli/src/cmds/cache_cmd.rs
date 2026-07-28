//! `source-cli cache` — HTML / EWMA cooldown / triage disk ops.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{json, Value};
use source_cache::{
    cooldown_for, get_html, get_triage, note_rate_limit, note_verify, put_html_bytes, put_triage,
    CachePaths,
};
use source_mcp::repo_root;

pub enum CacheCmd {
    GetHtml {
        url: String,
        max_age: f64,
        cache_dir: Option<PathBuf>,
    },
    PutHtml {
        url: String,
        body_file: PathBuf,
        meta_file: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
    },
    Cooldown {
        url: String,
        concurrent_rate: Option<String>,
        cache_dir: Option<PathBuf>,
    },
    NoteRateLimit {
        url: String,
        suggested_gap: f64,
        cache_dir: Option<PathBuf>,
    },
    NoteVerify {
        url: String,
        success: bool,
        duration_ms: u64,
        used_cooldown: f64,
        cache_dir: Option<PathBuf>,
    },
    GetTriage {
        url: String,
        max_age: f64,
        cache_dir: Option<PathBuf>,
    },
    PutTriage {
        url: String,
        report_file: PathBuf,
        cache_dir: Option<PathBuf>,
    },
}

fn paths(cache_dir: Option<PathBuf>) -> Result<CachePaths, String> {
    if let Some(dir) = cache_dir {
        Ok(CachePaths::from_cache_dir(dir))
    } else {
        let root = repo_root().map_err(|e| e.to_string())?;
        Ok(CachePaths::from_root(root))
    }
}

pub fn run_cache(cmd: CacheCmd) -> ExitCode {
    match run_cache_inner(cmd) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cache: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_cache_inner(cmd: CacheCmd) -> Result<Value, String> {
    match cmd {
        CacheCmd::GetHtml {
            url,
            max_age,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            match get_html(&p, &url, max_age).map_err(|e| e.to_string())? {
                Some(mut hit) => {
                    // Avoid dumping raw bytes to stdout; report length instead.
                    if let Some(obj) = hit.as_object_mut() {
                        if let Some(body) = obj.remove("body") {
                            let n = body.as_array().map(|a| a.len()).unwrap_or(0);
                            obj.insert("body_bytes".into(), json!(n));
                        }
                    }
                    Ok(hit)
                }
                None => Ok(json!({"cache_hit": false, "url": url})),
            }
        }
        CacheCmd::PutHtml {
            url,
            body_file,
            meta_file,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            let body = std::fs::read(&body_file).map_err(|e| e.to_string())?;
            let meta: Value = if let Some(mf) = meta_file {
                serde_json::from_str(&std::fs::read_to_string(mf).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?
            } else {
                json!({"ok": true, "status": 200, "bytes": body.len()})
            };
            let key = put_html_bytes(&p, &url, &body, &meta).map_err(|e| e.to_string())?;
            Ok(json!({"ok": true, "key": key, "bytes": body.len()}))
        }
        CacheCmd::Cooldown {
            url,
            concurrent_rate,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            let s =
                cooldown_for(&p, &url, concurrent_rate.as_deref()).map_err(|e| e.to_string())?;
            Ok(json!({"url": url, "cooldown_s": s}))
        }
        CacheCmd::NoteRateLimit {
            url,
            suggested_gap,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            let row = note_rate_limit(&p, &url, suggested_gap).map_err(|e| e.to_string())?;
            Ok(json!({"url": url, "host_stat": row}))
        }
        CacheCmd::NoteVerify {
            url,
            success,
            duration_ms,
            used_cooldown,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            let row = note_verify(&p, &url, success, duration_ms, used_cooldown)
                .map_err(|e| e.to_string())?;
            Ok(json!({"url": url, "host_stat": row}))
        }
        CacheCmd::GetTriage {
            url,
            max_age,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            match get_triage(&p, &url, max_age).map_err(|e| e.to_string())? {
                Some(v) => Ok(v),
                None => Ok(json!({"cache_hit": false, "url": url})),
            }
        }
        CacheCmd::PutTriage {
            url,
            report_file,
            cache_dir,
        } => {
            let p = paths(cache_dir)?;
            let report: Value = serde_json::from_str(
                &std::fs::read_to_string(&report_file).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            put_triage(&p, &url, &report).map_err(|e| e.to_string())?;
            Ok(json!({"ok": true, "url": url}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn put_get_html_roundtrip() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().join("cache");
        let body = dir.path().join("body.html");
        std::fs::write(&body, b"<html>hi</html>").unwrap();
        let _body_len = std::fs::metadata(&body).unwrap().len();
        let put = run_cache_inner(CacheCmd::PutHtml {
            url: "https://ex.test/a".into(),
            body_file: body,
            meta_file: None,
            cache_dir: Some(cache.clone()),
        })
        .unwrap();
        assert_eq!(put["ok"], true);
        let get = run_cache_inner(CacheCmd::GetHtml {
            url: "https://ex.test/a".into(),
            max_age: 3600.0,
            cache_dir: Some(cache),
        })
        .unwrap();
        assert_eq!(get["cache_hit"], true);
        assert_eq!(get["body_bytes"], b"<html>hi</html>".len());
    }
}
