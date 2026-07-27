//! LedgerPort — append-only session ledger.

use source_types::{LedgerRow, PortError};

pub trait LedgerPort {
    fn append(&self, row: &LedgerRow) -> Result<(), PortError>;
}
