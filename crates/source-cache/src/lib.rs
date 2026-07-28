//! Host EWMA cooldown + URL cache keys + disk HTML/triage
//! (parity with `repair_cache.py`).

mod disk;
mod ewma;
mod host_file;
mod keys;

pub use disk::{
    get_html, get_triage, put_html_bytes, put_triage, CacheIoError, CachePaths, IoResult,
};
pub use ewma::{
    apply_rate_limit, apply_verify_success, cooldown_seconds, HostStat, DEFAULT_GAP_S, EWMA_ALPHA,
};
pub use host_file::{cooldown_for, load_hosts, note_rate_limit, note_verify, save_hosts};
pub use keys::{host_of, url_key};
