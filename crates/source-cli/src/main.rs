//! CLI entry — keep thin; subcommands in cmds/, clap defs in cli.rs.

mod cli;
mod cli_subs;
mod cmds;

use clap::Parser;
use cli::{Cli, Cmd};
use cli_subs::{CheckSub, CloseoutSub, LedgerSub, ParseSub, ProgressSub, QueueSub, RetroSub};
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
        Cmd::Progress { cmd } => match cmd {
            ProgressSub::Status {
                index,
                rules,
                l0_only,
            } => run_progress(ProgressArgs {
                cmd: "status".into(),
                index,
                rules,
                l0_only,
            }),
            ProgressSub::Next {
                index,
                rules,
                l0_only,
            } => run_progress(ProgressArgs {
                cmd: "next".into(),
                index,
                rules,
                l0_only,
            }),
        },
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
        Cmd::Closeout { cmd } => match cmd {
            CloseoutSub::Pending => run_closeout(CloseoutArgs {
                cmd: "pending".into(),
                trap: None,
                skill_fix: false,
            }),
            CloseoutSub::Gate { trap, skill_fix } => run_closeout(CloseoutArgs {
                cmd: "gate".into(),
                trap: Some(trap),
                skill_fix,
            }),
            CloseoutSub::SyncSkill => run_closeout(CloseoutArgs {
                cmd: "sync-skill".into(),
                trap: None,
                skill_fix: false,
            }),
            CloseoutSub::Status => run_closeout(CloseoutArgs {
                cmd: "status".into(),
                trap: None,
                skill_fix: false,
            }),
        },
        Cmd::Retro { cmd } => match cmd {
            RetroSub::Append {
                url,
                status,
                msg,
                name,
                respond_time,
                waste_s,
                trap,
                harness,
                script_fix,
                skill_fix,
                seal,
            } => run_retro(RetroArgs {
                url,
                status,
                msg,
                name,
                respond_time,
                waste_s,
                trap,
                harness,
                script_fix,
                skill_fix,
                seal,
            }),
        },
        Cmd::Discover {
            write,
            timeout,
            config,
        } => run_discover(DiscoverArgs {
            write,
            timeout,
            config,
        }),
        Cmd::Check { cmd } => match cmd {
            CheckSub::Channel => run_check(CheckCmd::Channel),
            CheckSub::Precheck { urls_file, timeout } => {
                run_check(CheckCmd::Precheck { urls_file, timeout })
            }
            CheckSub::Batch {
                urls_file,
                keyword,
                batch_size,
                thread_count,
                timeout,
            } => run_check(CheckCmd::Batch {
                urls_file,
                keyword,
                batch_size,
                thread_count,
                timeout,
            }),
            CheckSub::Full {
                urls_file,
                keyword,
                batch_size,
                thread_count,
                timeout,
            } => run_check(CheckCmd::Full {
                urls_file,
                keyword,
                batch_size,
                thread_count,
                timeout,
            }),
        },
        Cmd::Queue { cmd } => match cmd {
            QueueSub::RefreshIndex { out } => run_queue(QueueCmd::RefreshIndex { out }),
            QueueSub::Rt {
                index,
                out,
                group,
                limit,
            } => run_queue(QueueCmd::Rt {
                index,
                out,
                group,
                limit,
            }),
        },
        Cmd::Parse { cmd } => match cmd {
            ParseSub::Rule { rule } => run_parse(ParseCmd::Rule { rule }),
            ParseSub::Url { url } => run_parse(ParseCmd::Url { url }),
        },
        Cmd::Parity { suite } => run_parity(ParityArgs { suite }),
        Cmd::Wave { urls_file, limit }
        | Cmd::Harvest { urls_file, limit }
        | Cmd::Serial { urls_file, limit } => run_orchestrate(OrchestrateArgs { urls_file, limit }),
        Cmd::Version => run_version(),
    }
}
