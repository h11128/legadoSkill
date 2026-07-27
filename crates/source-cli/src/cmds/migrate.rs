//! Domain migrate CLI.

use std::process::ExitCode;
use std::sync::Arc;

use source_mcp::{FsChannelPort, McpClient, McpEndpoint, McpSourceRepository};
use source_migrate::migrate_book_source;
use source_ports::{ChannelPort, SourceRepository};
use source_types::SourceKey;

pub struct MigrateArgs {
    pub from_url: String,
    pub to_url: String,
    pub dry_run: bool,
    pub keep_old: bool,
}

pub fn run_migrate(args: MigrateArgs) -> ExitCode {
    let ep = match McpEndpoint::load_defaults() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("migrate: {e}");
            return ExitCode::from(4);
        }
    };
    let client = Arc::new(McpClient::new(ep).with_client_name("source_cli_migrate"));
    if let Err(e) = client.ensure_session() {
        eprintln!("migrate: session: {e}");
        return ExitCode::from(2);
    }
    let channel = match FsChannelPort::from_repo() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("migrate: channel: {e}");
            return ExitCode::from(4);
        }
    };
    if let Err(e) = channel.assert_idle_for_repair() {
        eprintln!("migrate: {e}");
        return ExitCode::from(5);
    }
    let _g = match channel.acquire_repair() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("migrate: acquire: {e}");
            return ExitCode::from(5);
        }
    };
    let repo = McpSourceRepository::new(client);
    let src = match repo.get(&SourceKey::new(args.from_url.trim())) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("migrate: get: {e}");
            return ExitCode::from(2);
        }
    };
    let migrated = match migrate_book_source(&src, &args.from_url, &args.to_url) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("migrate: rewrite: {e}");
            return ExitCode::from(4);
        }
    };
    if args.dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(migrated.as_value()).unwrap_or_default()
        );
        return ExitCode::SUCCESS;
    }
    if let Err(e) = repo.save(&migrated) {
        eprintln!("migrate: save: {e}");
        return ExitCode::from(2);
    }
    if !args.keep_old {
        let _ = repo.delete(&[SourceKey::new(args.from_url.trim())]);
    }
    println!(
        "REPORT_JSON:{{\"schema_version\":\"1\",\"capability\":\"migrate\",\"mode\":\"oneshot\",\"url\":{},\"status\":\"migrated\",\"message\":\"migrated\",\"migrate_to\":{}}}",
        serde_json::to_string(args.from_url.trim()).unwrap(),
        serde_json::to_string(args.to_url.trim()).unwrap()
    );
    ExitCode::SUCCESS
}
