//! Debugger-style parse CLI.

use std::process::ExitCode;

use source_parse::{analyze_rule, analyze_url};

pub enum ParseCmd {
    Rule { rule: String },
    Url { url: String },
}

pub fn run_parse(cmd: ParseCmd) -> ExitCode {
    match cmd {
        ParseCmd::Rule { rule } => {
            println!("{}", analyze_rule(&rule));
            ExitCode::SUCCESS
        }
        ParseCmd::Url { url } => {
            println!("{}", analyze_url(&url));
            ExitCode::SUCCESS
        }
    }
}
