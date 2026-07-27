use source_probe::score_search_html;
use std::process::ExitCode;

pub fn run_probe_score(query: &str, status: u16, html: Option<String>) -> ExitCode {
    let body = html.unwrap_or_default();
    let s = score_search_html(&body, query, status);
    match serde_json::to_string(&s) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("probe-score: {e}");
            ExitCode::FAILURE
        }
    }
}
