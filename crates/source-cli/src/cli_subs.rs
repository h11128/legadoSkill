//! Additional clap subcommands (keeps cli.rs under line budget).

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum CloseoutSub {
    Pending,
    Gate {
        #[arg(long)]
        trap: String,
        #[arg(long, default_value_t = false)]
        skill_fix: bool,
    },
    SyncSkill,
    Status,
}

#[derive(Subcommand)]
pub enum LedgerSub {
    Append {
        #[arg(long)]
        url: String,
        #[arg(long)]
        step: String,
        #[arg(long)]
        result: String,
        #[arg(long)]
        note: Option<String>,
    },
    Show {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum ProgressSub {
    Status {
        #[arg(long)]
        index: Option<PathBuf>,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        l0_only: bool,
    },
    Next {
        #[arg(long)]
        index: Option<PathBuf>,
        #[arg(long)]
        rules: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        l0_only: bool,
    },
}

#[derive(Subcommand)]
pub enum CheckSub {
    Channel,
    Precheck {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value_t = 4.0)]
        timeout: f64,
    },
    Batch {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value = "我的")]
        keyword: String,
        #[arg(long, default_value_t = 80)]
        batch_size: usize,
        #[arg(long, default_value_t = 64)]
        thread_count: u32,
        #[arg(long, default_value_t = 45.0)]
        timeout: f64,
    },
    Full {
        #[arg(long)]
        urls_file: PathBuf,
        #[arg(long, default_value = "我的")]
        keyword: String,
        #[arg(long, default_value_t = 80)]
        batch_size: usize,
        #[arg(long, default_value_t = 64)]
        thread_count: u32,
        #[arg(long, default_value_t = 120.0)]
        timeout: f64,
    },
}

#[derive(Subcommand)]
pub enum QueueSub {
    RefreshIndex {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Rt {
        #[arg(long)]
        index: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "搜索失效")]
        group: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum RetroSub {
    Append {
        #[arg(long)]
        url: String,
        #[arg(long)]
        status: String,
        #[arg(long, default_value = "")]
        msg: String,
        #[arg(long, default_value = "")]
        name: String,
        #[arg(long)]
        respond_time: Option<i64>,
        #[arg(long, default_value_t = 0.0)]
        waste_s: f64,
        #[arg(long, default_value = "")]
        trap: String,
        #[arg(long, default_value = "")]
        harness: String,
        #[arg(long, default_value = "")]
        script_fix: String,
        #[arg(long, default_value_t = false)]
        skill_fix: bool,
        #[arg(long, default_value_t = true)]
        #[arg(long = "no-seal", action = clap::ArgAction::SetFalse)]
        seal: bool,
    },
}

#[derive(Subcommand)]
pub enum ParseSub {
    Rule {
        #[arg(long)]
        rule: String,
    },
    Url {
        #[arg(long)]
        url: String,
    },
}
