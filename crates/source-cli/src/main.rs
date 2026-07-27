mod cmds;

use clap::{Parser, Subcommand};
use cmds::{GateArgs, RepairArgs, RepairDryArgs};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "source-cli",
    about = "LegadoSkill repair platform CLI (Rust)",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Gate classify (full L0→L1→L2 when built with gate_full; else L0-only)
    Gate {
        #[arg(long)]
        url: String,
        #[arg(long)]
        rules: Option<PathBuf>,
        /// Skip L1/L2 network probes (always L0 denylist only)
        #[arg(long, default_value_t = false)]
        l0_only: bool,
        #[arg(long, default_value_t = 1.5)]
        tcp_timeout: f64,
        #[arg(long, default_value_t = 4.0)]
        l2_timeout: f64,
    },
    /// Spine dry-run oneshot (mem ports + AdapterRegistry; no MCP writes)
    RepairDry {
        #[arg(long)]
        url: String,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long, default_value_t = true)]
        l0_only: bool,
        #[arg(long, default_value_t = 1.5)]
        tcp_timeout: f64,
        #[arg(long, default_value_t = 4.0)]
        l2_timeout: f64,
        /// Prefetch HTML file for identify/adapters
        #[arg(long)]
        html: Option<PathBuf>,
    },
    /// Live oneshot: MCP get → adapters → save/verify (unless --dry-run / --no-verify)
    Repair {
        #[arg(long)]
        url: String,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        l0_only: bool,
        #[arg(long, default_value_t = 1.5)]
        tcp_timeout: f64,
        #[arg(long, default_value_t = 4.0)]
        l2_timeout: f64,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        no_verify: bool,
        /// HTML file instead of network prefetch
        #[arg(long)]
        html: Option<PathBuf>,
        /// Skip network prefetch of bookSourceUrl (identify may stay Unknown)
        #[arg(long, default_value_t = false)]
        no_prefetch: bool,
    },
    /// Pure EWMA math (parity helper)
    Ewma {
        #[arg(long, default_value_t = 3.0)]
        prev: f64,
        #[arg(long, default_value_t = 20.0)]
        suggested: f64,
    },
    /// Score search HTML from --html (or empty body)
    ProbeScore {
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 200)]
        status: u16,
        #[arg(long)]
        html: Option<String>,
    },
    /// Match URL against config/video_source_routes.json
    VideoRoute {
        #[arg(long)]
        url: String,
        #[arg(long)]
        routes: Option<PathBuf>,
    },
    /// Print crate versions and contract schema_version
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Gate {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
        } => cmds::run_gate(GateArgs {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
        }),
        Cmd::RepairDry {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            html,
        } => cmds::run_repair_dry(RepairDryArgs {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            html,
        }),
        Cmd::Repair {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            dry_run,
            no_verify,
            html,
            no_prefetch,
        } => cmds::run_repair(RepairArgs {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            dry_run,
            no_verify,
            html,
            prefetch: !no_prefetch,
        }),
        Cmd::Ewma { prev, suggested } => cmds::run_ewma(prev, suggested),
        Cmd::ProbeScore {
            query,
            status,
            html,
        } => cmds::run_probe_score(&query, status, html),
        Cmd::VideoRoute { url, routes } => cmds::run_video_route(&url, routes),
        Cmd::Version => cmds::run_version(),
    }
}
