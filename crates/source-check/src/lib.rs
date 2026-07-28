//! Bulk MCP check orchestration — port of batch_check_mcp / full_check_runner.

mod batch;
mod channel;
mod precheck;

pub use batch::{load_urls_file, run_batch_check, BatchCheckOpts, BatchCheckSummary};
pub use channel::channel_status_json as channel_status;
pub use precheck::{precheck_json, precheck_urls, PrecheckRow};
