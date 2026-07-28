//! Early-exit when live probe marks form search endpoints dead.

use super::repair_outcome::RepairOneOutcome;
use source_ports::{Clock, LedgerPort};
use source_types::{LedgerRow, LedgerStep, Url};

pub fn report_search_endpoint_dead<L: LedgerPort, C: Clock>(
    url: &str,
    ledger: &L,
    clock: &C,
) -> RepairOneOutcome {
    if let Ok(u) = Url::new(url) {
        let row = LedgerRow::new(
            clock.now_utc().to_rfc3339(),
            u,
            LedgerStep::Skip,
            "search_endpoint_dead",
        );
        let _ = ledger.append(&row);
    }
    RepairOneOutcome::skipped(url, "search_endpoint_dead")
}
