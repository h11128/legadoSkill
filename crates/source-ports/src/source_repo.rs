//! SourceRepository — MCP get/save/disable/delete boundary.

use source_types::{BookSource, PortError, SourceKey};

pub trait SourceRepository {
    fn get(&self, key: &SourceKey) -> Result<BookSource, PortError>;
    fn save(&self, source: &BookSource) -> Result<(), PortError>;
    fn disable(&self, key: &SourceKey) -> Result<(), PortError>;
    fn delete(&self, keys: &[SourceKey]) -> Result<(), PortError>;
}
