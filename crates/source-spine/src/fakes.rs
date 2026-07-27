//! In-memory port fakes for spine unit / integration tests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use source_ports::{ChannelGuard, ChannelPort, Clock, LedgerPort, SourceRepository, VerifyPort};
use source_types::{
    BookSource, CheckOpts, LedgerRow, Mode, PortError, SourceKey, VerifyResult,
};

#[derive(Debug, Default)]
pub struct MemRepo {
    pub by_key: RefCell<HashMap<String, BookSource>>,
    pub save_fail: RefCell<bool>,
}

impl MemRepo {
    pub fn with_source(source: BookSource) -> Self {
        let key = source
            .source_key()
            .map(|k| k.as_str().to_string())
            .unwrap_or_default();
        let mut map = HashMap::new();
        map.insert(key, source);
        Self {
            by_key: RefCell::new(map),
            save_fail: RefCell::new(false),
        }
    }
}

impl SourceRepository for MemRepo {
    fn get(&self, key: &SourceKey) -> Result<BookSource, PortError> {
        self.by_key
            .borrow()
            .get(key.as_str())
            .cloned()
            .ok_or_else(|| PortError::Permanent(format!("missing {}", key.as_str())))
    }

    fn save(&self, source: &BookSource) -> Result<(), PortError> {
        if *self.save_fail.borrow() {
            return Err(PortError::Transient("save forced fail".into()));
        }
        let key = source
            .source_key()
            .ok_or_else(|| PortError::ContractViolation("no bookSourceUrl".into()))?;
        self.by_key
            .borrow_mut()
            .insert(key.as_str().to_string(), source.clone());
        Ok(())
    }

    fn disable(&self, _key: &SourceKey) -> Result<(), PortError> {
        Ok(())
    }

    fn delete(&self, _keys: &[SourceKey]) -> Result<(), PortError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct MemVerify {
    pub success: RefCell<bool>,
    pub message: RefCell<String>,
    pub calls: RefCell<u32>,
}

impl Default for MemVerify {
    fn default() -> Self {
        Self {
            success: RefCell::new(true),
            message: RefCell::new("ok".into()),
            calls: RefCell::new(0),
        }
    }
}

impl VerifyPort for MemVerify {
    fn check(&self, key: &SourceKey, opts: CheckOpts) -> Result<VerifyResult, PortError> {
        *self.calls.borrow_mut() += 1;
        let _ = opts; // repair default: check_discovery=false
        let url = key
            .to_url()
            .map_err(|e| PortError::ContractViolation(e.to_string()))?;
        Ok(VerifyResult::new(
            url,
            *self.success.borrow(),
            self.message.borrow().clone(),
            Mode::Oneshot,
        ))
    }
}

#[derive(Debug, Default)]
pub struct MemLedger {
    pub rows: RefCell<Vec<LedgerRow>>,
}

impl LedgerPort for MemLedger {
    fn append(&self, row: &LedgerRow) -> Result<(), PortError> {
        self.rows.borrow_mut().push(row.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct IdleGuard;

impl ChannelGuard for IdleGuard {}

#[derive(Debug, Default)]
pub struct IdleChannel;

impl ChannelPort for IdleChannel {
    type Guard = IdleGuard;

    fn assert_idle_for_repair(&self) -> Result<(), PortError> {
        Ok(())
    }

    fn acquire_repair(&self) -> Result<Self::Guard, PortError> {
        Ok(IdleGuard)
    }
}

#[derive(Debug, Default)]
pub struct BusyChannel;

impl ChannelPort for BusyChannel {
    type Guard = IdleGuard;

    fn assert_idle_for_repair(&self) -> Result<(), PortError> {
        Err(PortError::ChannelBusy("bulk holds MCP".into()))
    }

    fn acquire_repair(&self) -> Result<Self::Guard, PortError> {
        Err(PortError::ChannelBusy("bulk holds MCP".into()))
    }
}

#[derive(Debug, Clone)]
pub struct FixedClock(pub DateTime<Utc>);

impl Default for FixedClock {
    fn default() -> Self {
        Self(Utc::now())
    }
}

impl Clock for FixedClock {
    fn now_utc(&self) -> DateTime<Utc> {
        self.0
    }

    fn sleep(&self, _d: Duration) {}
}
