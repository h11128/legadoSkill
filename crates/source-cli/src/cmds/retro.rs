//! Retro append CLI with optional ledger seal.

use std::process::ExitCode;

use source_closeout::{append_retro, sync_skill_to_cursor, CloseoutPaths, RetroAppendOpts};

pub struct RetroArgs {
    pub url: String,
    pub status: String,
    pub msg: String,
    pub name: String,
    pub respond_time: Option<i64>,
    pub waste_s: f64,
    pub trap: String,
    pub harness: String,
    pub script_fix: String,
    pub skill_fix: bool,
    pub seal: bool,
}

pub fn run_retro(args: RetroArgs) -> ExitCode {
    let paths = match CloseoutPaths::from_repo() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("retro: {e}");
            return ExitCode::from(4);
        }
    };
    let opts = RetroAppendOpts {
        url: args.url,
        status: args.status,
        msg: args.msg,
        name: args.name,
        respond_time: args.respond_time,
        waste_s: args.waste_s,
        trap: args.trap,
        harness: args.harness,
        script_fix: args.script_fix,
        skill_fix: args.skill_fix,
        seal: args.seal,
    };
    match append_retro(&paths, opts) {
        Ok(row) => {
            if let Some(sealed) = &row.sealed {
                println!(
                    "sealed ledger: {} {}",
                    sealed.get("step").and_then(|v| v.as_str()).unwrap_or(""),
                    sealed.get("result").and_then(|v| v.as_str()).unwrap_or("")
                );
            }
            let skill_fix = row
                .row
                .get("skill_fix")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if skill_fix {
                match sync_skill_to_cursor(&paths) {
                    Ok(msg) => println!("synced SKILL → {msg}"),
                    Err(e) => {
                        eprintln!("retro: skill sync failed: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            println!("{}", row.row);
            ExitCode::SUCCESS
        }
        Err(errs) => {
            for e in errs {
                eprintln!("retro BLOCK: {e}");
            }
            ExitCode::from(1)
        }
    }
}
