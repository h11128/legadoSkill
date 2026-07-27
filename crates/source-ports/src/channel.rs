//! ChannelPort — exclusive repair vs bulk MCP lock.

use source_types::PortError;

/// RAII-ish repair channel guard. Drop releases the lock in infra adapters.
pub trait ChannelGuard: Send {}

pub trait ChannelPort {
    type Guard: ChannelGuard;

    fn assert_idle_for_repair(&self) -> Result<(), PortError>;
    fn acquire_repair(&self) -> Result<Self::Guard, PortError>;
}
