//! MCP HTTP client + port adapters for legadoSkill (architecture §14.2 / Phase B–C).
//!
//! Python SOT: `scripts/mcp_client.py`, `scripts/mcp_channel.py`,
//! `scripts/repair_wait.py`, `scripts/repair_session_log.py`.

mod channel;
mod client;
mod endpoint;
mod fakes;
mod ledger;
mod root;
mod source_repo;
mod verify;

pub use channel::{FsChannelGuard, FsChannelPort};
pub use client::McpClient;
pub use endpoint::McpEndpoint;
pub use fakes::{
    MemChannelGuard, MemChannelPort, MemClock, MemLedgerPort, MemSourceRepository, MemVerifyPort,
};
pub use ledger::{
    default_jsonl_path, default_sqlite_path, DualLedgerPort, JsonlLedgerPort, SqliteLedgerPort,
};
pub use root::repo_root;
pub use source_repo::{url_candidates, McpSourceRepository};
pub use verify::McpVerifyPort;

#[cfg(test)]
mod live_tests {
    use std::sync::Arc;

    use source_ports::{SourceRepository, VerifyPort};
    use source_types::{CheckOpts, SourceKey};

    use super::*;

    /// Live phone MCP — run with `cargo test -p source_mcp -- --ignored`.
    #[test]
    #[ignore = "requires reachable phone MCP from mcp_defaults.json"]
    fn live_get_save_roundtrip_smoke() {
        let ep = McpEndpoint::load_defaults().expect("defaults");
        let client = Arc::new(McpClient::new(ep).with_client_name("source_mcp_live"));
        client.ensure_session().expect("session");
        let repo = McpSourceRepository::new(client);
        // Intentionally use a URL unlikely to exist — expect Permanent, not transport panic.
        let key = SourceKey::new("https://source-mcp-live-missing.example/");
        let err = repo.get(&key).expect_err("missing source");
        let _ = format!("{err}");
    }

    #[test]
    #[ignore = "requires reachable phone MCP; starts a device check"]
    fn live_verify_smoke() {
        let ep = McpEndpoint::load_defaults().expect("defaults");
        let client = Arc::new(McpClient::new(ep).with_client_name("source_mcp_live"));
        let verify = McpVerifyPort::new(client).with_max_wait_s(30.0);
        let key = SourceKey::new("https://source-mcp-live-missing.example/");
        let _ = verify.check(&key, CheckOpts::default());
    }
}
