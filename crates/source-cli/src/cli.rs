//! Clap CLI definition (kept out of main.rs for the 300-line limit).

use crate::cli_subs::{
    CheckSub, CloseoutSub, LedgerSub, ParseSub, ProgressSub, QueueSub, RetroSub,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "source-cli",
    about = "LegadoSkill repair platform CLI",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    Gate {
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
    },
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
        #[arg(long)]
        html: Option<PathBuf>,
    },
    Repair {
        #[arg(long, default_value = "")]
        url: String,
        #[arg(long)]
        urls_file: Option<PathBuf>,
        #[arg(long, default_value = "oneshot", value_parser = ["oneshot", "batch"])]
        mode: String,
        #[arg(long, default_value_t = 15)]
        limit: usize,
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
        #[arg(long)]
        html: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        no_prefetch: bool,
        #[arg(long, default_value = "我的")]
        key: String,
        #[arg(long, default_value_t = false)]
        skip_diagnose: bool,
    },
    Diagnose {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "我的")]
        key: String,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        l0_only: bool,
        #[arg(long, default_value_t = 1.5)]
        tcp_timeout: f64,
        #[arg(long, default_value_t = 4.0)]
        l2_timeout: f64,
        #[arg(long)]
        debug_file: Option<PathBuf>,
    },
    Probe {
        #[arg(long)]
        base_url: String,
        #[arg(long)]
        html: Option<String>,
        #[arg(long)]
        html_file: Option<PathBuf>,
        #[arg(long, default_value = "我的")]
        key: String,
    },
    Migrate {
        #[arg(long = "from-url")]
        from_url: String,
        #[arg(long = "to-url")]
        to_url: String,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        keep_old: bool,
    },
    Hunt {
        #[arg(long)]
        url: String,
        #[arg(long)]
        seeds: Option<PathBuf>,
    },
    Progress {
        #[command(subcommand)]
        cmd: ProgressSub,
    },
    Ledger {
        #[command(subcommand)]
        cmd: LedgerSub,
    },
    Ewma {
        #[arg(long, default_value_t = 3.0)]
        prev: f64,
        #[arg(long, default_value_t = 20.0)]
        suggested: f64,
    },
    ProbeScore {
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 200)]
        status: u16,
        #[arg(long)]
        html: Option<String>,
    },
    VideoRoute {
        #[arg(long)]
        url: String,
        #[arg(long)]
        routes: Option<PathBuf>,
    },
    Closeout {
        #[command(subcommand)]
        cmd: CloseoutSub,
    },
    Retro {
        #[command(subcommand)]
        cmd: RetroSub,
    },
    Discover {
        #[arg(long, default_value_t = false)]
        write: bool,
        #[arg(long, default_value_t = 5.0)]
        timeout: f64,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Check {
        #[command(subcommand)]
        cmd: CheckSub,
    },
    Queue {
        #[command(subcommand)]
        cmd: QueueSub,
    },
    Parse {
        #[command(subcommand)]
        cmd: ParseSub,
    },
    Parity {
        #[arg(long)]
        suite: Option<String>,
    },
    Wave {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value_t = 15)]
        limit: usize,
    },
    Harvest {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Serial {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Version,
}
