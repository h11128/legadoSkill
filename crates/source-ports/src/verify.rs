//! VerifyPort — device MCP check.

use source_types::{CheckOpts, PortError, SourceKey, VerifyResult};

pub trait VerifyPort {
    /// Device check. `CheckOpts::check_discovery` defaults to false.
    fn check(&self, key: &SourceKey, opts: CheckOpts) -> Result<VerifyResult, PortError>;
}
