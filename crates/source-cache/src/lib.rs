//! Host EWMA cooldown + URL cache keys (parity with `repair_cache.py`).

mod ewma;
mod keys;

pub use ewma::{apply_rate_limit, apply_verify_success, cooldown_seconds, HostStat};
pub use keys::{host_of, url_key};
