//! Early-exit when live probe marks form search endpoints dead.

use source_ports::{Clock, LedgerPort};
use source_types::{LedgerRow, LedgerStep, Url};
use std::process::ExitCode;

pub fn report_search_endpoint_dead<L: LedgerPort, C: Clock>(
    url: &str,
    ledger: &L,
    clock: &C,
) -> ExitCode {
    let line = format!(
        "REPORT_JSON:{{\"schema_version\":\"1\",\"capability\":\"repair\",\"mode\":\"oneshot\",\"url\":{},\"status\":\"skipped\",\"message\":\"search_endpoint_dead\"}}",
        serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into())
    );
    println!("{line}");
    if let Ok(u) = Url::new(url) {
        let row = LedgerRow::new(
            clock.now_utc().to_rfc3339(),
            u,
            LedgerStep::Skip,
            "search_endpoint_dead",
        );
        let _ = ledger.append(&row);
    }
    ExitCode::SUCCESS
}
