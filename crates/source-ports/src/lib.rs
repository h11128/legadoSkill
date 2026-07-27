//! Hexagonal ports for the repair platform (§14.2).
//!
//! Depends only on `source_types` (+ chrono for `Clock`). No rusqlite/reqwest.

mod channel;
mod clock;
mod html_fetch;
mod ledger;
mod source_repo;
mod verify;

pub use channel::{ChannelGuard, ChannelPort};
pub use clock::Clock;
pub use html_fetch::HtmlFetchPort;
pub use ledger::LedgerPort;
pub use source_repo::SourceRepository;
pub use verify::VerifyPort;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use source_types::{
        BookSource, CheckOpts, FetchResult, HeaderMap, LedgerRow, LedgerStep, PortError, SourceKey,
        Url, VerifyResult,
    };
    use std::cell::RefCell;
    use std::time::Duration;

    struct MemRepo {
        stored: RefCell<Option<BookSource>>,
    }

    impl SourceRepository for MemRepo {
        fn get(&self, _key: &SourceKey) -> Result<BookSource, PortError> {
            self.stored
                .borrow()
                .clone()
                .ok_or_else(|| PortError::Permanent("missing".into()))
        }

        fn save(&self, source: &BookSource) -> Result<(), PortError> {
            *self.stored.borrow_mut() = Some(source.clone());
            Ok(())
        }

        fn disable(&self, _key: &SourceKey) -> Result<(), PortError> {
            Ok(())
        }

        fn delete(&self, _keys: &[SourceKey]) -> Result<(), PortError> {
            Ok(())
        }
    }

    struct NoopVerify;
    impl VerifyPort for NoopVerify {
        fn check(
            &self,
            key: &SourceKey,
            opts: CheckOpts,
        ) -> Result<VerifyResult, PortError> {
            assert!(!opts.check_discovery);
            Ok(VerifyResult::new(
                key.to_url().map_err(|e| PortError::ContractViolation(e.to_string()))?,
                true,
                "ok",
                source_types::Mode::Oneshot,
            ))
        }
    }

    struct NoopFetch;
    impl HtmlFetchPort for NoopFetch {
        fn fetch(&self, url: &Url, _headers: &HeaderMap) -> Result<FetchResult, PortError> {
            Ok(FetchResult::new(200, url.clone(), Vec::new()))
        }
    }

    struct MemLedger {
        rows: RefCell<Vec<LedgerRow>>,
    }
    impl LedgerPort for MemLedger {
        fn append(&self, row: &LedgerRow) -> Result<(), PortError> {
            self.rows.borrow_mut().push(row.clone());
            Ok(())
        }
    }

    struct IdleGuard;
    impl ChannelGuard for IdleGuard {}

    struct IdleChannel;
    impl ChannelPort for IdleChannel {
        type Guard = IdleGuard;
        fn assert_idle_for_repair(&self) -> Result<(), PortError> {
            Ok(())
        }
        fn acquire_repair(&self) -> Result<Self::Guard, PortError> {
            Ok(IdleGuard)
        }
    }

    struct FixedClock(DateTime<Utc>);
    impl Clock for FixedClock {
        fn now_utc(&self) -> DateTime<Utc> {
            self.0
        }
        fn sleep(&self, _d: Duration) {}
    }

    #[test]
    fn fake_ports_compile_and_run() {
        let url = Url::new("https://example.com/").unwrap();
        let key = SourceKey::new(url.as_str());
        let repo = MemRepo {
            stored: RefCell::new(None),
        };
        let src = BookSource::new(serde_json::json!({"bookSourceUrl": url.as_str()}));
        repo.save(&src).unwrap();
        assert_eq!(repo.get(&key).unwrap().source_key().unwrap(), key);

        let v = NoopVerify.check(&key, CheckOpts::default()).unwrap();
        assert!(v.success);

        let _body = NoopFetch.fetch(&url, &HeaderMap::new()).unwrap();
        let ledger = MemLedger {
            rows: RefCell::new(Vec::new()),
        };
        ledger
            .append(&LedgerRow::new("2026-07-26T00:00:00Z", url, LedgerStep::Check, "ok"))
            .unwrap();
        assert_eq!(ledger.rows.borrow().len(), 1);

        IdleChannel.assert_idle_for_repair().unwrap();
        let _g = IdleChannel.acquire_repair().unwrap();
        let _now = FixedClock(Utc::now()).now_utc();
    }
}
