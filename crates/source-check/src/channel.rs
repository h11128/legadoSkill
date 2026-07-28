//! MCP channel status JSON (repair vs bulk lock).

use serde_json::Value;
use source_mcp::channel_status;
use source_types::PortError;

pub fn channel_status_json() -> Result<Value, PortError> {
    channel_status()
}
