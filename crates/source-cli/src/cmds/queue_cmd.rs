//! Queue / phone index / remain-cluster CLI.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{json, Value};
use source_db::Db;
use source_mcp::default_sqlite_path;
use source_pattern::{cluster_remain, sample_from_value, samples_from_json, ClusterSample};
use source_queue::{
    build_rt_queue, build_rt_queue_full, default_serial_queue_path, refresh_phone_index,
    write_rt_queue, RtBuildOpts,
};
use source_types::RepairConfig;

pub enum QueueCmd {
    RefreshIndex {
        out: Option<PathBuf>,
    },
    Rt {
        index: Option<PathBuf>,
        out: Option<PathBuf>,
        group: String,
        limit: usize,
        max_rt_ms: i64,
        full: bool,
        all_sources: Option<PathBuf>,
        ledger: Option<PathBuf>,
    },
    Cluster {
        queue: Option<PathBuf>,
        sources_file: Option<PathBuf>,
        db: Option<PathBuf>,
        min_size: u32,
        out: Option<PathBuf>,
        from_mcp: bool,
        limit: usize,
    },
}

pub fn run_queue(cmd: QueueCmd) -> ExitCode {
    match cmd {
        QueueCmd::RefreshIndex { out } => match refresh_phone_index(out) {
            Ok(r) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "path": r.path,
                        "total": r.total,
                        "cache_hit": r.cache_hit,
                    })
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("queue refresh-index: {e}");
                ExitCode::from(1)
            }
        },
        QueueCmd::Rt {
            index,
            out,
            group,
            limit,
            max_rt_ms,
            full,
            all_sources,
            ledger,
        } => {
            let index =
                index.unwrap_or_else(|| PathBuf::from("temp/full_fix/phone_source_index.json"));
            let out_path = out.unwrap_or_else(|| {
                default_serial_queue_path().unwrap_or_else(|_| {
                    PathBuf::from("temp/full_fix/queues/repair_serial100_queue.json")
                })
            });
            if full {
                match build_rt_queue_full(
                    &index,
                    &RtBuildOpts {
                        max_rt_ms,
                        limit,
                        enabled_only: true,
                        search_tag_only: group.contains("搜索"),
                        all_sources_path: all_sources,
                        ledger_path: ledger,
                    },
                ) {
                    Ok(doc) => {
                        if let Some(parent) = out_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        if let Ok(raw) = serde_json::to_string_pretty(&doc) {
                            if fs::write(&out_path, raw).is_err() {
                                eprintln!("queue rt write: {}", out_path.display());
                                return ExitCode::from(1);
                            }
                        }
                        println!(
                            "{}",
                            json!({
                                "path": out_path,
                                "n_selected": doc.get("n_selected"),
                                "max_rt_ms": max_rt_ms,
                                "full": true,
                            })
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("queue rt full: {e}");
                        ExitCode::from(1)
                    }
                }
            } else {
                match build_rt_queue(&index, &group) {
                    Ok(items) => match write_rt_queue(&out_path, &items, limit) {
                        Ok(doc) => {
                            println!(
                                "{}",
                                json!({
                                    "path": out_path,
                                    "total": doc.get("total"),
                                    "written": doc.get("written"),
                                })
                            );
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("queue rt write: {e}");
                            ExitCode::from(1)
                        }
                    },
                    Err(e) => {
                        eprintln!("queue rt: {e}");
                        ExitCode::from(1)
                    }
                }
            }
        }
        QueueCmd::Cluster {
            queue,
            sources_file,
            db,
            min_size,
            out,
            from_mcp,
            limit,
        } => match run_cluster(queue, sources_file, db, min_size, out, from_mcp, limit) {
            Ok(v) => {
                println!("{v}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("queue cluster: {e}");
                ExitCode::from(1)
            }
        },
    }
}

fn run_cluster(
    queue: Option<PathBuf>,
    sources_file: Option<PathBuf>,
    db_path: Option<PathBuf>,
    min_size: u32,
    out: Option<PathBuf>,
    from_mcp: bool,
    limit: usize,
) -> Result<Value, String> {
    let cfg = RepairConfig {
        cluster_min_size: min_size.max(1),
        ..Default::default()
    };

    let queue_urls = load_queue_urls(queue)?;
    let mut samples: Vec<ClusterSample> = Vec::new();

    if let Some(path) = &sources_file {
        let raw = fs::read_to_string(path).map_err(|e| format!("read sources: {e}"))?;
        let doc: Value = serde_json::from_str(&raw).map_err(|e| format!("json: {e}"))?;
        samples.extend(samples_from_json(&doc, false));
    }

    let db_path = db_path.or_else(|| default_sqlite_path().ok());
    if let Some(ref path) = db_path {
        let db = Db::connect(path).map_err(|e| e.to_string())?;
        if sources_file.is_none() && queue_urls.is_empty() {
            for (_k, v) in db
                .list_snapshot_payloads(false)
                .map_err(|e| e.to_string())?
            {
                if let Some(s) = sample_from_value(&v, false) {
                    samples.push(s);
                }
            }
        } else if !queue_urls.is_empty() {
            for u in &queue_urls {
                if let Some(v) = db.get_snapshot_payload(u).map_err(|e| e.to_string())? {
                    if let Some(s) = sample_from_value(&v, false) {
                        samples.push(s);
                    }
                }
            }
        }
    }

    let cached_n = samples.len();
    let mut fetched_n = 0usize;

    if from_mcp {
        let have: std::collections::HashSet<_> =
            samples.iter().map(|s| s.url.as_str().to_string()).collect();
        let mut need: Vec<String> = if !queue_urls.is_empty() {
            queue_urls
                .iter()
                .filter(|u| !have.contains(u.as_str()))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        if limit > 0 {
            need.truncate(limit);
        }
        if !need.is_empty() {
            let fetched = fetch_mcp_samples(&need)?;
            fetched_n = fetched.len();
            if let Some(ref path) = db_path {
                if let Ok(db) = Db::connect(path) {
                    for s in &fetched {
                        let _ = upsert_sample(&db, s);
                    }
                }
            }
            samples.extend(fetched);
        }
    }

    if !queue_urls.is_empty() {
        let want: std::collections::HashSet<_> = queue_urls.iter().cloned().collect();
        samples.retain(|s| want.contains(s.url.as_str()));
    }
    if limit > 0 && samples.len() > limit {
        samples.truncate(limit);
    }

    let report = cluster_remain(&samples, &cfg);
    let v = serde_json::to_value(&report).map_err(|e| e.to_string())?;
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(
            &path,
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(json!({
        "ok": true,
        "cached_n": cached_n,
        "fetched_n": fetched_n,
        "missing_payload_n": queue_urls.len().saturating_sub(samples.len()),
        "report": v,
    }))
}

fn load_queue_urls(queue: Option<PathBuf>) -> Result<Vec<String>, String> {
    let Some(path) = queue else {
        return Ok(Vec::new());
    };
    let raw = fs::read_to_string(&path).map_err(|e| format!("read queue: {e}"))?;
    let doc: Value = serde_json::from_str(&raw).map_err(|e| format!("queue json: {e}"))?;
    let mut urls = Vec::new();
    if let Some(items) = doc.get("items").and_then(|v| v.as_array()) {
        for it in items {
            if let Some(u) = it.get("url").and_then(|x| x.as_str()) {
                urls.push(u.to_string());
            } else if let Some(u) = it.as_str() {
                urls.push(u.to_string());
            }
        }
    } else if let Some(arr) = doc.as_array() {
        for it in arr {
            if let Some(u) = it.get("url").and_then(|x| x.as_str()) {
                urls.push(u.to_string());
            } else if let Some(u) = it.as_str() {
                urls.push(u.to_string());
            }
        }
    }
    Ok(urls)
}

fn fetch_mcp_samples(urls: &[String]) -> Result<Vec<ClusterSample>, String> {
    use source_mcp::{McpClient, McpEndpoint, McpSourceRepository};
    use source_ports::SourceRepository;
    use source_types::SourceKey;
    use std::sync::Arc;

    if urls.is_empty() {
        return Ok(Vec::new());
    }
    let ep = McpEndpoint::load_defaults().map_err(|e| e.to_string())?;
    let client = Arc::new(McpClient::new(ep).with_client_name("queue_cluster"));
    client.ensure_session().map_err(|e| e.to_string())?;
    let repo = McpSourceRepository::new(client);
    let mut out = Vec::new();
    for u in urls {
        match repo.get(&SourceKey::new(u)) {
            Ok(src) => {
                if let Some(s) = sample_from_value(src.as_value(), false) {
                    out.push(s);
                }
            }
            Err(e) => eprintln!("queue cluster get_source {u}: {e}"),
        }
    }
    Ok(out)
}

fn upsert_sample(db: &Db, sample: &ClusterSample) -> Result<(), String> {
    use source_db::{host_key, iso_now, norm_source_key, SourceSnapshotRow};
    let key = norm_source_key(sample.url.as_str());
    let payload = serde_json::to_string(sample.source.as_value()).map_err(|e| e.to_string())?;
    let v = sample.source.as_value();
    let row = SourceSnapshotRow {
        source_key: key.clone(),
        host_key: host_key(&key),
        name: v
            .get("bookSourceName")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        book_source_type: v
            .get("bookSourceType")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        group_name: v
            .get("bookSourceGroup")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        respond_time_ms: v.get("respondTime").and_then(|x| x.as_i64()),
        payload_json: payload,
        pulled_at: iso_now(),
    };
    db.upsert_source_snapshot(&row).map_err(|e| e.to_string())
}
