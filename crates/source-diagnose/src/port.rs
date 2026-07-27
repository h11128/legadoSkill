//! DiagnosePort adapter over offline parse.

use source_ports::DiagnosePort;
use source_types::{BookSource, DiagnoseResult, Url};

use crate::engine::diagnose_from_debug;

#[derive(Debug, Default, Clone, Copy)]
pub struct ParseDiagnosePort;

impl DiagnosePort for ParseDiagnosePort {
    fn diagnose(
        &self,
        url: Url,
        _source: &BookSource,
        debug_text: &str,
        fail_msg: Option<&str>,
    ) -> DiagnoseResult {
        diagnose_from_debug(url, debug_text, None, fail_msg)
    }
}
