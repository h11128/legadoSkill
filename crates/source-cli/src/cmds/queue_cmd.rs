//! Queue / phone index CLI.

use std::path::PathBuf;
use std::process::ExitCode;

use source_queue::{
    build_rt_queue, default_serial_queue_path, refresh_phone_index, write_rt_queue,
};

pub enum QueueCmd {
    RefreshIndex {
        out: Option<PathBuf>,
    },
    Rt {
        index: Option<PathBuf>,
        out: Option<PathBuf>,
        group: String,
        limit: usize,
    },
}

pub fn run_queue(cmd: QueueCmd) -> ExitCode {
    match cmd {
        QueueCmd::RefreshIndex { out } => match refresh_phone_index(out) {
            Ok(r) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "path": r.path,
                        "total": r.total,
                        "cache_hit": r.cache_hit,
                    })
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("queue refresh-index: {e}");
                ExitCode::from(1)
            }
        },
        QueueCmd::Rt {
            index,
            out,
            group,
            limit,
        } => {
            let index =
                index.unwrap_or_else(|| PathBuf::from("temp/full_fix/phone_source_index.json"));
            let out_path = out.unwrap_or_else(|| {
                default_serial_queue_path().unwrap_or_else(|_| {
                    PathBuf::from("temp/full_fix/queues/repair_serial100_queue.json")
                })
            });
            match build_rt_queue(&index, &group) {
                Ok(items) => match write_rt_queue(&out_path, &items, limit) {
                    Ok(doc) => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "path": out_path,
                                "total": doc.get("total"),
                                "written": doc.get("written"),
                            })
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("queue rt write: {e}");
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("queue rt: {e}");
                    ExitCode::from(1)
                }
            }
        }
    }
}
