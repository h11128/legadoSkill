//! Idempotent §9.3 schema migration.

use rusqlite::Connection;

use crate::Result;

pub const SCHEMA_VERSION: &str = "1";

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_snapshot (
  source_key   TEXT PRIMARY KEY,
  host_key     TEXT NOT NULL,
  name         TEXT,
  type         INTEGER NOT NULL DEFAULT 0,
  enabled      INTEGER NOT NULL,
  family       TEXT,
  structural_hash TEXT,
  group_name   TEXT,
  respond_time_ms INTEGER,
  payload_json TEXT NOT NULL,
  pulled_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_source_host ON source_snapshot(host_key);
CREATE INDEX IF NOT EXISTS idx_source_family ON source_snapshot(family);

CREATE TABLE IF NOT EXISTS ledger_events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  ts           TEXT NOT NULL,
  source_key   TEXT NOT NULL,
  step         TEXT NOT NULL,
  result       TEXT NOT NULL,
  note         TEXT,
  waste        TEXT,
  capability   TEXT,
  family       TEXT,
  layer        TEXT,
  report_status TEXT,
  row_json     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ledger_source_ts ON ledger_events(source_key, ts);
CREATE INDEX IF NOT EXISTS idx_ledger_status ON ledger_events(report_status, ts);

CREATE TABLE IF NOT EXISTS gate_runs (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  ts           TEXT NOT NULL,
  source_key   TEXT NOT NULL,
  action       TEXT NOT NULL,
  reason       TEXT NOT NULL,
  migrate_to   TEXT,
  result_json  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS verify_runs (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  ts           TEXT NOT NULL,
  source_key   TEXT NOT NULL,
  success      INTEGER NOT NULL,
  message      TEXT,
  mode         TEXT,
  check_discovery INTEGER NOT NULL DEFAULT 0,
  duration_ms  INTEGER,
  capability   TEXT,
  result_json  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_verify_source ON verify_runs(source_key, ts);

CREATE TABLE IF NOT EXISTS host_stats (
  host_key     TEXT PRIMARY KEY,
  ewma_gap_s   REAL NOT NULL DEFAULT 3.0,
  hits         INTEGER NOT NULL DEFAULT 0,
  ok           INTEGER NOT NULL DEFAULT 0,
  fail         INTEGER NOT NULL DEFAULT 0,
  rate_limits  INTEGER NOT NULL DEFAULT 0,
  last_rate_limit_at REAL,
  last_duration_ms INTEGER,
  last_at      REAL,
  extra_json   TEXT
);

CREATE TABLE IF NOT EXISTS html_cache_meta (
  cache_key    TEXT PRIMARY KEY,
  url          TEXT NOT NULL,
  host_key     TEXT NOT NULL,
  saved_at     REAL NOT NULL,
  status       INTEGER,
  final_url    TEXT,
  content_type TEXT,
  bytes        INTEGER,
  rate_limited INTEGER,
  bin_path     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_html_host ON html_cache_meta(host_key, saved_at);

CREATE TABLE IF NOT EXISTS pattern_cluster (
  family       TEXT PRIMARY KEY,
  size         INTEGER NOT NULL,
  structural_hash TEXT,
  confidence   REAL,
  centroid_json TEXT NOT NULL,
  exemplars_json TEXT NOT NULL,
  coverage_json TEXT,
  extracted_at TEXT NOT NULL,
  promoted     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS claims (
  source_key   TEXT NOT NULL,
  status       TEXT NOT NULL,
  ts           TEXT NOT NULL,
  evidence     TEXT,
  agent        TEXT,
  root_cause   TEXT,
  PRIMARY KEY (source_key, status, ts)
);
"#;

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(DDL)?;
    conn.execute(
        "INSERT INTO schema_meta(key, value) VALUES('version', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [SCHEMA_VERSION],
    )?;
    Ok(())
}
