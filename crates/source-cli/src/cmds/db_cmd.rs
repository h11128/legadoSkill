//! `source-cli db` — migrate / status / import / export-phone-index.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use source_db::{db_path, load_cfg, Db};
use source_mcp::repo_root;

pub enum DbCmd {
    Migrate,
    Status,
    ImportLedger { path: PathBuf },
    ImportHtmlCache { dir: PathBuf },
    ImportHostStats { path: PathBuf },
    ImportCache,
    ExportPhoneIndex { out: PathBuf },
}

fn open_db() -> Result<(Db, source_db::RepairDbCfg, PathBuf, PathBuf), String> {
    let root = repo_root().map_err(|e| e.to_string())?;
    let cfg = load_cfg(&root);
    let path = db_path(&root, &cfg);
    let db = Db::connect(&path).map_err(|e| e.to_string())?;
    Ok((db, cfg, root, path))
}

fn resolve_under_root(root: &std::path::Path, p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

pub fn run_db(cmd: DbCmd) -> ExitCode {
    match run_db_inner(cmd) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("db: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_db_inner(cmd: DbCmd) -> Result<serde_json::Value, String> {
    let (db, cfg, root, path) = open_db()?;
    match cmd {
        DbCmd::Migrate => Ok(json!({"db": path.display().to_string(), "ok": true})),
        DbCmd::Status => db
            .status_json(&path, cfg.phone_index_ttl_s)
            .map_err(|e| e.to_string()),
        DbCmd::ImportLedger { path: p } => {
            let p = resolve_under_root(&root, p);
            let n = db.import_jsonl_ledger(&p).map_err(|e| e.to_string())?;
            Ok(json!({"imported": n, "path": p.display().to_string()}))
        }
        DbCmd::ImportHtmlCache { dir } => {
            let dir = resolve_under_root(&root, dir);
            let n = db.import_html_cache_dir(&dir).map_err(|e| e.to_string())?;
            Ok(json!({"imported": n, "dir": dir.display().to_string()}))
        }
        DbCmd::ImportHostStats { path: p } => {
            let p = resolve_under_root(&root, p);
            let n = db.import_host_stats_file(&p).map_err(|e| e.to_string())?;
            Ok(json!({"imported": n, "path": p.display().to_string()}))
        }
        DbCmd::ImportCache => {
            let html_dir = root.join("temp/full_fix/cache/html");
            let host = root.join("temp/full_fix/cache/host_stats.json");
            let (html_n, host_n) = db
                .import_cache(&html_dir, &host)
                .map_err(|e| e.to_string())?;
            Ok(json!({"html_meta": html_n, "host_stats": host_n}))
        }
        DbCmd::ExportPhoneIndex { out } => {
            let out = resolve_under_root(&root, out);
            let payload = db
                .export_phone_index_json(&out)
                .map_err(|e| e.to_string())?;
            Ok(json!({
                "total": payload.get("total"),
                "out": out.display().to_string(),
                "from_db": true,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn migrate_creates_sqlite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("repair_state.sqlite");
        let db = Db::connect(&path).unwrap();
        let st = db.status_json(&path, 3600.0).unwrap();
        assert_eq!(st["source_snapshots"], 0);
        assert_eq!(st["ledger_events"], 0);
    }
}
