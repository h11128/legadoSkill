//! Unit tests for `source_db::Db` facade.

use super::*;
use serde_json::json;
use source_types::{LedgerStep, Mode, Url};
use tempfile::TempDir;

fn open_tmp() -> (TempDir, Db) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("repair_state.sqlite");
    let db = Db::connect(&path).expect("connect");
    (dir, db)
}

#[test]
fn migrate_is_idempotent() {
    let (_dir, db) = open_tmp();
    db.migrate_schema().expect("migrate again");
    let n: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
               'source_snapshot','ledger_events','gate_runs','verify_runs',
               'host_stats','html_cache_meta','pattern_cluster','claims','schema_meta'
             )",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 9);
}

#[test]
fn import_ledger_and_status() {
    let (dir, db) = open_tmp();
    let ledger = dir.path().join("ledger.jsonl");
    std::fs::write(
        &ledger,
        r#"{"ts":"2026-07-27T00:00:00Z","url":"https://a.example/","step":"gate","result":"ok"}
{"ts":"2026-07-27T00:00:00Z","url":"https://a.example/","step":"gate","result":"ok"}
"#,
    )
    .unwrap();
    assert_eq!(db.import_jsonl_ledger(&ledger).unwrap(), 1);
    let st = db
        .status_json(dir.path().join("repair_state.sqlite"), 3600.0)
        .unwrap();
    assert_eq!(st["ledger_events"], 1);
}

#[test]
fn phone_export_and_freshness() {
    let (dir, db) = open_tmp();
    let n = db
        .bulk_upsert_list_items(&[json!({
            "bookSourceUrl": "https://a.example/",
            "bookSourceName": "A",
            "enabled": true,
            "respondTime": 100
        })])
        .unwrap();
    assert_eq!(n, 1);
    assert!(db.phone_index_fresh(3600.0).unwrap());
    let out = dir.path().join("phone.json");
    let payload = db.export_phone_index_json(&out).unwrap();
    assert_eq!(payload["total"], 1);
    assert!(out.is_file());
}

#[test]
fn import_html_and_host_stats() {
    let (dir, db) = open_tmp();
    let html = dir.path().join("html");
    std::fs::create_dir_all(&html).unwrap();
    let key = "abc123";
    std::fs::write(
        html.join(format!("{key}.json")),
        r#"{"url":"https://a.example/x","saved_at":1.0,"status":200,"bytes":3}"#,
    )
    .unwrap();
    std::fs::write(html.join(format!("{key}.bin")), b"hi!").unwrap();
    let hosts = dir.path().join("host_stats.json");
    std::fs::write(
        &hosts,
        r#"{"a.example":{"ewma_gap_s":4.0,"hits":2,"ok":1,"fail":1,"rate_limits":0}}"#,
    )
    .unwrap();
    let (hn, sn) = db.import_cache(&html, &hosts).unwrap();
    assert_eq!(hn, 1);
    assert_eq!(sn, 1);
}

#[test]
fn snapshot_ttl_fresh() {
    let (_dir, db) = open_tmp();
    let row = SourceSnapshotRow {
        source_key: "https://a.example/".into(),
        host_key: "a.example".into(),
        name: Some("Demo".into()),
        book_source_type: 0,
        enabled: true,
        group_name: None,
        respond_time_ms: Some(1),
        payload_json: r#"{"bookSourceUrl":"https://a.example/"}"#.into(),
        pulled_at: iso_now(),
    };
    db.upsert_source_snapshot(&row).unwrap();
    assert!(db
        .get_source_payload_fresh("https://a.example/", 600.0)
        .unwrap()
        .is_some());
    assert!(db
        .get_source_payload_fresh("https://a.example/", 0.0)
        .unwrap()
        .is_none());
}

#[test]
fn append_ledger_roundtrip() {
    let (_dir, db) = open_tmp();
    let url = Url::new("https://a.example/").unwrap();
    let row = LedgerRow::new("2026-07-27T00:00:00Z", url, LedgerStep::Gate, "skip");
    let id = db.append_ledger(&row).unwrap();
    assert!(id > 0);
}

#[test]
fn upsert_host_stats_and_verify() {
    let (_dir, db) = open_tmp();
    let host = HostStatsRow {
        host_key: "a.example".into(),
        ewma_gap_s: 4.5,
        hits: 2,
        ok: 1,
        fail: 1,
        rate_limits: 0,
        last_rate_limit_at: None,
        last_duration_ms: Some(120),
        last_at: Some(1.0),
        extra_json: None,
    };
    db.upsert_host_stats(&host).unwrap();
    let url = Url::new("https://a.example/").unwrap();
    let mut vr = VerifyResult::new(url, true, "ok", Mode::Oneshot);
    vr.duration_ms = Some(200);
    assert!(db.record_verify(&vr).unwrap() > 0);
}
