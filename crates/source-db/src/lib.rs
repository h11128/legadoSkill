//! SQLite persistence (§9): WAL, foreign_keys, migrate + ledger/host/verify APIs.
//! Feature parity for `repair_db*.py` (library surface; CLI wiring is separate).

mod cfg;
mod error;
mod host_stats;
mod html_meta;
mod import;
mod keys;
mod ledger;
mod pattern_store;
mod phone;
mod schema;
mod source_snapshot;
mod verify;

pub use cfg::{
    cfg_from_value, db_path, load_cfg, RepairDbCfg, DEFAULT_PHONE_TTL_S, DEFAULT_SNAPSHOT_TTL_S,
};
pub use error::DbError;
pub use host_stats::HostStatsRow;
pub use html_meta::{html_meta_count, import_html_cache_dir, upsert_html_meta, HtmlMetaRow};
pub use import::{import_host_stats_file, import_jsonl_ledger, ledger_event_count};
pub use keys::{host_key, iso_now, norm_source_key};
pub use pattern_store::{
    count_clusters, fixed_source_keys, get_payload, list_payloads, upsert_cluster,
};
pub use phone::{bulk_upsert_list_items, export_phone_index_json, phone_index_fresh, status_json};
pub use source_snapshot::{
    count as snapshot_count, get as get_snapshot, get_fresh_payload, upsert as upsert_snapshot,
    SourceSnapshotRow,
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
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let db = Self { conn };
        db.migrate_schema()?;
        Ok(db)
    }

    /// Connect using repo root + `repair_db_defaults.json`.
    pub fn connect_defaults(root: impl AsRef<Path>) -> Result<(Self, RepairDbCfg)> {
        let root = root.as_ref();
        let cfg = load_cfg(root);
        let path = db_path(root, &cfg);
        Ok((Self::connect(&path)?, cfg))
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

    pub fn upsert_source_snapshot(&self, row: &SourceSnapshotRow) -> Result<()> {
        source_snapshot::upsert(&self.conn, row)
    }

    pub fn get_source_snapshot(&self, source_key: &str) -> Result<Option<SourceSnapshotRow>> {
        source_snapshot::get(&self.conn, source_key)
    }

    /// TTL-aware payload (Python `get_source_snapshot`).
    pub fn get_source_payload_fresh(
        &self,
        source_key: &str,
        max_age_s: f64,
    ) -> Result<Option<serde_json::Value>> {
        source_snapshot::get_fresh_payload(&self.conn, source_key, max_age_s)
    }

    pub fn source_snapshot_count(&self) -> Result<i64> {
        source_snapshot::count(&self.conn)
    }

    pub fn delete_source_snapshot(&self, source_key: &str) -> Result<()> {
        source_snapshot::delete(&self.conn, source_key)
    }

    pub fn list_snapshot_payloads(
        &self,
        enabled_only: bool,
    ) -> Result<Vec<(String, serde_json::Value)>> {
        pattern_store::list_payloads(&self.conn, enabled_only)
    }

    pub fn fixed_source_keys(&self) -> Result<Vec<String>> {
        pattern_store::fixed_source_keys(&self.conn)
    }

    pub fn upsert_pattern_cluster(&self, cluster: &source_types::PatternCluster) -> Result<()> {
        pattern_store::upsert_cluster(&self.conn, cluster)
    }

    pub fn pattern_cluster_count(&self) -> Result<i64> {
        pattern_store::count_clusters(&self.conn)
    }

    pub fn get_snapshot_payload(&self, source_key: &str) -> Result<Option<serde_json::Value>> {
        pattern_store::get_payload(&self.conn, source_key)
    }

    pub fn upsert_html_meta_row(&self, row: &HtmlMetaRow) -> Result<()> {
        html_meta::upsert_html_meta(&self.conn, row)
    }

    pub fn import_jsonl_ledger(&self, path: impl AsRef<Path>) -> Result<usize> {
        import::import_jsonl_ledger(&self.conn, path.as_ref())
    }

    pub fn import_html_cache_dir(&self, html_dir: impl AsRef<Path>) -> Result<usize> {
        html_meta::import_html_cache_dir(&self.conn, html_dir.as_ref())
    }

    pub fn import_host_stats_file(&self, path: impl AsRef<Path>) -> Result<usize> {
        import::import_host_stats_file(&self.conn, path.as_ref())
    }

    pub fn import_cache(
        &self,
        html_dir: impl AsRef<Path>,
        host_stats: impl AsRef<Path>,
    ) -> Result<(usize, usize)> {
        let html_n = self.import_html_cache_dir(html_dir)?;
        let host_n = self.import_host_stats_file(host_stats)?;
        Ok((html_n, host_n))
    }

    pub fn bulk_upsert_list_items(&self, items: &[serde_json::Value]) -> Result<usize> {
        phone::bulk_upsert_list_items(&self.conn, items)
    }

    pub fn phone_index_fresh(&self, ttl_s: f64) -> Result<bool> {
        phone::phone_index_fresh(&self.conn, ttl_s)
    }

    pub fn export_phone_index_json(&self, out: impl AsRef<Path>) -> Result<serde_json::Value> {
        phone::export_phone_index_json(&self.conn, out.as_ref())
    }

    pub fn status_json(
        &self,
        db_path: impl AsRef<Path>,
        phone_ttl_s: f64,
    ) -> Result<serde_json::Value> {
        phone::status_json(&self.conn, db_path.as_ref(), phone_ttl_s)
    }

    /// Borrow the underlying connection (tests / advanced callers).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
