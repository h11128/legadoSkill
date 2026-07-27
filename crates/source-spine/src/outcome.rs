//! ApplyOutcome and optional idempotency store (§14.5).

use source_types::{BookSource, ReportJson, VerifyResult};

/// Outcome of one apply attempt (never claims fixed without verify.success).
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub idempotency_key: String,
    pub before: BookSource,
    pub after: Option<BookSource>,
    pub dry_run: bool,
    pub saved: bool,
    pub verify: Option<VerifyResult>,
    pub report: ReportJson,
    pub report_line: String,
    pub exit_code: i32,
    /// Set when verify failed after a successful save (§14.5).
    pub verify_failed_after_save: bool,
}

/// Optional short-circuit: same idempotency key already verified ok.
pub trait IdempotencyStore {
    fn last_verify_ok(&self, key: &str) -> bool;
    fn remember_ok(&mut self, key: &str);
}

/// In-memory set for tests / optional oneshot cache.
#[derive(Debug, Default, Clone)]
pub struct MemoryIdempotency {
    ok: std::collections::HashSet<String>,
}

impl IdempotencyStore for MemoryIdempotency {
    fn last_verify_ok(&self, key: &str) -> bool {
        self.ok.contains(key)
    }

    fn remember_ok(&mut self, key: &str) {
        self.ok.insert(key.to_string());
    }
}
