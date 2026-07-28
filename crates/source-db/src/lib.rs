//! SQLite persistence (§9): WAL, foreign_keys, migrate + ledger/host/verify APIs.

mod error;
mod host_stats;
mod ledger;
mod schema;
mod source_snapshot;
mod verify;

pub use error::DbError;
pub use host_stats::HostStatsRow;
pub use source_snapshot::{
    count as snapshot_count, get as get_snapshot, upsert as upsert_snapshot, SourceSnapshotRow,
};

use rusqlite::Connection;
use std::path::Path;

use source_types::{LedgerRow, VerifyResult};

pub type Result<T> = std::result::Result<T, DbError>;

/// Opened repair-state database (architecture §9.4).
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open or create SQLite at `path`, apply WAL + foreign_keys, migrate schema.
    pub fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let db = Self { conn };
        db.migrate_schema()?;
        Ok(db)
    }

    /// Idempotent CREATE IF NOT EXISTS for §9.3 tables + schema_meta version.
    pub fn migrate_schema(&self) -> Result<()> {
        schema::migrate(&self.conn)
    }

    pub fn append_ledger(&self, row: &LedgerRow) -> Result<i64> {
        ledger::append(&self.conn, row)
    }

    pub fn upsert_host_stats(&self, row: &HostStatsRow) -> Result<()> {
        host_stats::upsert(&self.conn, row)
    }

    pub fn record_verify(&self, result: &VerifyResult) -> Result<i64> {
        verify::record(&self.conn, result)
    }

    #[allow(dead_code)]
    fn upsert_source_snapshot(&self, row: &SourceSnapshotRow) -> Result<()> {
        source_snapshot::upsert(&self.conn, row)
    }

    pub fn get_source_snapshot(&self, source_key: &str) -> Result<Option<SourceSnapshotRow>> {
        source_snapshot::get(&self.conn, source_key)
    }

    pub fn source_snapshot_count(&self) -> Result<i64> {
        source_snapshot::count(&self.conn)
    }

    /// Borrow the underlying connection (tests / advanced callers).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let ver: String = db
            .connection()
            .query_row(
                "SELECT value FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ver, "1");
    }

    #[test]
    fn append_ledger_roundtrip() {
        let (_dir, db) = open_tmp();
        let url = Url::new("https://a.example/").unwrap();
        let row = LedgerRow::new("2026-07-27T00:00:00Z", url, LedgerStep::Gate, "skip");
        let id = db.append_ledger(&row).unwrap();
        assert!(id > 0);
        let (sk, step): (String, String) = db
            .connection()
            .query_row(
                "SELECT source_key, step FROM ledger_events WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(sk, "https://a.example/");
        assert_eq!(step, "gate");
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
        db.upsert_host_stats(&HostStatsRow {
            hits: 3,
            ..host.clone()
        })
        .unwrap();
        let hits: i64 = db
            .connection()
            .query_row(
                "SELECT hits FROM host_stats WHERE host_key='a.example'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 3);

        let url = Url::new("https://a.example/").unwrap();
        let mut vr = VerifyResult::new(url, true, "ok", Mode::Oneshot);
        vr.duration_ms = Some(200);
        let id = db.record_verify(&vr).unwrap();
        assert!(id > 0);
        let success: i64 = db
            .connection()
            .query_row("SELECT success FROM verify_runs WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(success, 1);
    }

    #[test]
    fn wal_and_foreign_keys_on() {
        let (_dir, db) = open_tmp();
        let mode: String = db
            .connection()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");
        let fk: i64 = db
            .connection()
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn source_snapshot_roundtrip() {
        let (_dir, db) = open_tmp();
        let row = SourceSnapshotRow {
            source_key: "https://a.example/".into(),
            host_key: "a.example".into(),
            name: Some("Demo".into()),
            book_source_type: 0,
            enabled: true,
            group_name: Some("未整理".into()),
            respond_time_ms: Some(1200),
            payload_json: r#"{"bookSourceUrl":"https://a.example/"}"#.into(),
            pulled_at: "2026-07-27T00:00:00Z".into(),
        };
        db.upsert_source_snapshot(&row).unwrap();
        assert_eq!(db.source_snapshot_count().unwrap(), 1);
        let got = db
            .get_source_snapshot("https://a.example/")
            .unwrap()
            .unwrap();
        assert_eq!(got.name.as_deref(), Some("Demo"));
    }
}
