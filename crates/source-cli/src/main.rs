//! CLI entry — keep thin; subcommands in cmds/, clap defs in cli.rs.

mod cli;
mod cmds;

use clap::Parser;
use cli::{Cli, Cmd, LedgerSub};
use cmds::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Gate {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
        } => run_gate(GateArgs {
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
        } => run_repair_dry(RepairDryArgs {
            url,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            html,
        }),
        Cmd::Repair {
            url,
            urls_file,
            mode,
            limit,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            dry_run,
            no_verify,
            html,
            no_prefetch,
            key,
            skip_diagnose,
        } => run_repair(RepairArgs {
            url,
            urls_file,
            mode,
            limit,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            dry_run,
            no_verify,
            html,
            prefetch: !no_prefetch,
            key,
            skip_diagnose,
        }),
        Cmd::Diagnose {
            url,
            key,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            debug_file,
        } => run_diagnose(DiagnoseArgs {
            url,
            key,
            rules,
            l0_only,
            tcp_timeout,
            l2_timeout,
            debug_file,
        }),
        Cmd::Probe {
            base_url,
            html,
            html_file,
            key,
        } => run_probe(ProbeArgs {
            base_url,
            html,
            html_file,
            key,
        }),
        Cmd::Migrate {
            from_url,
            to_url,
            dry_run,
            keep_old,
        } => run_migrate(MigrateArgs {
            from_url,
            to_url,
            dry_run,
            keep_old,
        }),
        Cmd::Hunt { url, seeds } => run_hunt(HuntArgs { url, seeds }),
        Cmd::Progress {
            cmd,
            index,
            rules,
            l0_only,
        } => run_progress(ProgressArgs {
            cmd,
            index,
            rules,
            l0_only,
        }),
        Cmd::Ledger { cmd } => match cmd {
            LedgerSub::Append {
                url,
                step,
                result,
                note,
            } => run_ledger(LedgerCmd::Append {
                url,
                step,
                result,
                note,
            }),
            LedgerSub::Show { limit } => run_ledger(LedgerCmd::Show { limit }),
        },
        Cmd::Ewma { prev, suggested } => run_ewma(prev, suggested),
        Cmd::ProbeScore {
            query,
            status,
            html,
        } => run_probe_score(&query, status, html),
        Cmd::VideoRoute { url, routes } => run_video_route(&url, routes),
        Cmd::Version => run_version(),
    }
}
