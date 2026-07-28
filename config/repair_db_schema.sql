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
