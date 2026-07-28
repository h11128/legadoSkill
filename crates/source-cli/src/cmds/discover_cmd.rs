//! MCP discover CLI.

use std::path::PathBuf;
use std::process::ExitCode;

use source_mcp::{discover, repo_root, write_discover_defaults};

pub struct DiscoverArgs {
    pub write: bool,
    pub timeout: f64,
    pub config: Option<PathBuf>,
}

pub fn run_discover(args: DiscoverArgs) -> ExitCode {
    match discover(args.timeout) {
        Ok(found) => {
            println!("{}", found);
            if args.write {
                let root = match repo_root() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("discover: {e}");
                        return ExitCode::from(4);
                    }
                };
                let path = args
                    .config
                    .unwrap_or_else(|| root.join("config/mcp_defaults.json"));
                if let Err(e) = write_discover_defaults(&found, &path) {
                    eprintln!("discover write: {e}");
                    return ExitCode::from(1);
                }
            }
            if found.get("found") == Some(&serde_json::json!(true)) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("discover: {e}");
            ExitCode::from(1)
        }
    }
}
