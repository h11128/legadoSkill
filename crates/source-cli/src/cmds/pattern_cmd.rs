//! `source-cli pattern extract` — verify-ok structural clustering (§4.2).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use serde_json::{json, Value};
use source_db::Db;
use source_mcp::{default_sqlite_path, repo_root};
use source_pattern::{
    cluster_verify_ok, sample_from_value, samples_from_json, write_cluster_json, ClusterSample,
};
use source_types::{BookSource, RepairConfig};

pub enum PatternCmd {
    Extract {
        sources_file: Option<PathBuf>,
        db: Option<PathBuf>,
        out_dir: Option<PathBuf>,
        min_size: u32,
        limit: usize,
        write_db: bool,
        from_mcp: bool,
        enabled_only: bool,
        fixed_only: bool,
    },
}

pub fn run_pattern(cmd: PatternCmd) -> ExitCode {
    match cmd {
        PatternCmd::Extract {
            sources_file,
            db,
            out_dir,
            min_size,
            limit,
            write_db,
            from_mcp,
            enabled_only,
            fixed_only,
        } => match extract(
            sources_file,
            db,
            out_dir,
            min_size,
            limit,
            write_db,
            from_mcp,
            enabled_only,
            fixed_only,
        ) {
            Ok(v) => {
                println!("{}", v);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("pattern extract: {e}");
                ExitCode::from(1)
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn extract(
    sources_file: Option<PathBuf>,
    db_path: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    min_size: u32,
    limit: usize,
    write_db: bool,
    from_mcp: bool,
    enabled_only: bool,
    fixed_only: bool,
) -> Result<Value, String> {
    let cfg = RepairConfig {
        cluster_min_size: min_size.max(1),
        ..Default::default()
    };
    let db_path = db_path
        .or_else(|| default_sqlite_path().ok())
        .ok_or_else(|| "no db path".to_string())?;
    let db = Db::connect(&db_path).map_err(|e| e.to_string())?;

    let mut samples: Vec<ClusterSample> = Vec::new();
    if let Some(path) = sources_file {
        let raw = fs::read_to_string(&path).map_err(|e| format!("read sources: {e}"))?;
        let doc: Value = serde_json::from_str(&raw).map_err(|e| format!("json: {e}"))?;
        samples.extend(samples_from_json(&doc, true));
    } else {
        samples.extend(samples_from_db(&db, enabled_only, fixed_only)?);
    }

    if from_mcp {
        let need = missing_fixed_urls(&db, &samples, limit)?;
        if !need.is_empty() {
            let fetched = fetch_mcp_samples(&need)?;
            for s in &fetched {
                let _ = upsert_sample(&db, s);
            }
            samples.extend(fetched);
        }
    }

    // Dedupe by url; prefer verify_ok=true.
    samples = dedupe_samples(samples);
    if limit > 0 && samples.len() > limit {
        samples.truncate(limit);
    }

    let extracted_at = Utc::now().to_rfc3339();
    let clusters = cluster_verify_ok(&samples, &cfg, &extracted_at);

    let mut written = Vec::new();
    if let Some(dir) = out_dir {
        let root = repo_root().map_err(|e| e.to_string())?;
        let dir = if dir.is_absolute() {
            dir
        } else {
            root.join(dir)
        };
        for c in &clusters {
            let p = write_cluster_json(c, &dir).map_err(|e| e.to_string())?;
            written.push(p.display().to_string());
        }
    }
    if write_db {
        for c in &clusters {
            db.upsert_pattern_cluster(c).map_err(|e| e.to_string())?;
        }
    }

    Ok(json!({
        "ok": true,
        "input_n": samples.len(),
        "verify_ok_n": samples.iter().filter(|s| s.verify_ok).count(),
        "min_size": cfg.cluster_min_size,
        "cluster_n": clusters.len(),
        "db_cluster_n": db.pattern_cluster_count().unwrap_or(0),
        "written": written,
        "clusters": clusters.iter().map(|c| json!({
            "family": c.family.as_str(),
            "size": c.size,
            "structural_hash": c.fingerprint.structural_hash,
            "confidence": c.fingerprint.confidence,
            "exemplars": c.exemplars.iter().map(|u| u.as_str()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    }))
}

fn samples_from_db(
    db: &Db,
    enabled_only: bool,
    fixed_only: bool,
) -> Result<Vec<ClusterSample>, String> {
    let fixed: std::collections::HashSet<String> = if fixed_only {
        db.fixed_source_keys()
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    let rows = db
        .list_snapshot_payloads(enabled_only)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (key, v) in rows {
        if fixed_only
            && !fixed.contains(&key)
            && !fixed
                .iter()
                .any(|f| key.starts_with(f) || f.starts_with(&key))
        {
            continue;
        }
        let verify_ok = fixed_only || fixed.contains(&key);
        if let Some(mut s) = sample_from_value(&v, verify_ok) {
            if fixed_only {
                s.verify_ok = true;
            }
            out.push(s);
        }
    }
    Ok(out)
}

fn missing_fixed_urls(
    db: &Db,
    have: &[ClusterSample],
    limit: usize,
) -> Result<Vec<String>, String> {
    let have_set: std::collections::HashSet<_> =
        have.iter().map(|s| s.url.as_str().to_string()).collect();
    let mut need = Vec::new();
    for key in db.fixed_source_keys().map_err(|e| e.to_string())? {
        if have_set.contains(&key) {
            continue;
        }
        if db
            .get_snapshot_payload(&key)
            .map_err(|e| e.to_string())?
            .and_then(|v| sample_from_value(&v, true))
            .is_some()
        {
            continue;
        }
        need.push(key);
        if limit > 0 && need.len() >= limit {
            break;
        }
    }
    Ok(need)
}

fn fetch_mcp_samples(urls: &[String]) -> Result<Vec<ClusterSample>, String> {
    use source_mcp::{McpClient, McpEndpoint, McpSourceRepository};
    use source_ports::SourceRepository;
    use source_types::SourceKey;
    use std::sync::Arc;

    let ep = McpEndpoint::load_defaults().map_err(|e| e.to_string())?;
    let client = Arc::new(McpClient::new(ep).with_client_name("pattern_extract"));
    client.ensure_session().map_err(|e| e.to_string())?;
    let repo = McpSourceRepository::new(client);
    let mut out = Vec::new();
    for u in urls {
        let key = SourceKey::new(u);
        match repo.get(&key) {
            Ok(src) => {
                if let Some(s) = sample_from_value(src.as_value(), true) {
                    out.push(s);
                }
            }
            Err(e) => eprintln!("pattern extract get_source {u}: {e}"),
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
    db.upsert_source_snapshot(&row).map_err(|e| e.to_string())?;
    let _ = BookSource::new(v.clone());
    Ok(())
}

fn dedupe_samples(samples: Vec<ClusterSample>) -> Vec<ClusterSample> {
    use std::collections::HashMap;
    let mut map: HashMap<String, ClusterSample> = HashMap::new();
    for s in samples {
        let k = s.url.as_str().to_string();
        match map.get(&k) {
            Some(old) if old.verify_ok && !s.verify_ok => {}
            _ => {
                map.insert(k, s);
            }
        }
    }
    let mut out: Vec<_> = map.into_values().collect();
    out.sort_by(|a, b| a.url.as_str().cmp(b.url.as_str()));
    out
}
