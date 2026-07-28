//! In-memory port fakes for `source-spine` unit tests.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use source_ports::{ChannelGuard, ChannelPort, Clock, LedgerPort, SourceRepository, VerifyPort};
use source_types::{BookSource, CheckOpts, LedgerRow, Mode, PortError, SourceKey, VerifyResult};

/// Thread-safe in-memory `SourceRepository`.
pub struct MemSourceRepository {
    store: Mutex<HashMap<String, BookSource>>,
}

impl MemSourceRepository {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_source(source: BookSource) -> Self {
        let repo = Self::new();
        let _ = repo.save(&source);
        repo
    }
}

impl Default for MemSourceRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRepository for MemSourceRepository {
    fn get(&self, key: &SourceKey) -> Result<BookSource, PortError> {
        self.store
            .lock()
            .map_err(|_| PortError::Transient("poisoned".into()))?
            .get(key.as_str())
            .cloned()
            .ok_or_else(|| PortError::Permanent(format!("missing source {}", key.as_str())))
    }

    fn save(&self, source: &BookSource) -> Result<(), PortError> {
        let key = source
            .source_key()
            .ok_or_else(|| PortError::ContractViolation("bookSourceUrl missing".into()))?;
        self.store
            .lock()
            .map_err(|_| PortError::Transient("poisoned".into()))?
            .insert(key.as_str().to_string(), source.clone());
        Ok(())
    }

    fn disable(&self, key: &SourceKey) -> Result<(), PortError> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| PortError::Transient("poisoned".into()))?;
        let src = store
            .get_mut(key.as_str())
            .ok_or_else(|| PortError::Permanent(format!("missing source {}", key.as_str())))?;
        let mut v = src.clone().into_value();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("enabled".into(), serde_json::json!(false));
        }
        *src = BookSource::new(v);
        Ok(())
    }

    fn delete(&self, keys: &[SourceKey]) -> Result<(), PortError> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| PortError::Transient("poisoned".into()))?;
        for k in keys {
            store.remove(k.as_str());
        }
        Ok(())
    }
}

/// Always-success (or configurable) verify fake.
pub struct MemVerifyPort {
    pub success: bool,
    pub message: String,
}

impl MemVerifyPort {
    pub fn ok() -> Self {
        Self {
            success: true,
            message: "ok".into(),
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

impl Default for MemVerifyPort {
    fn default() -> Self {
        Self::ok()
    }
}

impl VerifyPort for MemVerifyPort {
    fn check(&self, key: &SourceKey, opts: CheckOpts) -> Result<VerifyResult, PortError> {
        let url = key
            .to_url()
            .map_err(|e| PortError::ContractViolation(e.to_string()))?;
        let mut vr = VerifyResult::new(url, self.success, self.message.clone(), Mode::Oneshot);
        vr.check_discovery = opts.check_discovery;
        Ok(vr)
    }
}

/// Collects ledger rows in memory.
pub struct MemLedgerPort {
    rows: Mutex<Vec<LedgerRow>>,
}

impl MemLedgerPort {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
        }
    }

    pub fn rows(&self) -> Vec<LedgerRow> {
        self.rows.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl Default for MemLedgerPort {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerPort for MemLedgerPort {
    fn append(&self, row: &LedgerRow) -> Result<(), PortError> {
        self.rows
            .lock()
            .map_err(|_| PortError::Transient("poisoned".into()))?
            .push(row.clone());
        Ok(())
    }
}

/// No-op channel guard.
pub struct MemChannelGuard;

impl ChannelGuard for MemChannelGuard {}

/// Idle channel (never busy).
pub struct MemChannelPort;

impl ChannelPort for MemChannelPort {
    type Guard = MemChannelGuard;

    fn assert_idle_for_repair(&self) -> Result<(), PortError> {
        Ok(())
    }

    fn acquire_repair(&self) -> Result<Self::Guard, PortError> {
        Ok(MemChannelGuard)
    }
}

/// Fixed clock for deterministic tests.
pub struct MemClock {
    pub now: DateTime<Utc>,
}

impl MemClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }
}

impl Clock for MemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        self.now
    }

    fn sleep(&self, _d: Duration) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_types::Url;

    #[test]
    fn mem_repo_roundtrip() {
        let url = "https://mem.example/";
        let src = BookSource::new(serde_json::json!({
            "bookSourceUrl": url,
            "bookSourceName": "mem",
            "enabled": true,
        }));
        let repo = MemSourceRepository::with_source(src);
        let key = SourceKey::new(url);
        assert_eq!(repo.get(&key).unwrap().source_key().unwrap(), key);
        repo.disable(&key).unwrap();
        let disabled = repo.get(&key).unwrap();
        assert_eq!(disabled.as_value()["enabled"], false);
        repo.delete(std::slice::from_ref(&key)).unwrap();
        assert!(repo.get(&key).is_err());
    }

    #[test]
    fn mem_verify_and_ledger() {
        let key = SourceKey::new("https://mem.example/");
        let v = MemVerifyPort::ok()
            .check(&key, CheckOpts::default())
            .unwrap();
        assert!(v.success);
        let ledger = MemLedgerPort::new();
        ledger
            .append(&LedgerRow::new(
                "2026-07-27T00:00:00Z",
                Url::new("https://mem.example/").unwrap(),
                source_types::LedgerStep::Check,
                "ok",
            ))
            .unwrap();
        assert_eq!(ledger.rows().len(), 1);
        MemChannelPort.assert_idle_for_repair().unwrap();
        let _g = MemChannelPort.acquire_repair().unwrap();
    }
}
