//! Diagnose: debug_parse + layer engine (architecture §3.3 / Phase 1 parity).

mod debug_parse;
mod engine;
mod port;

pub use debug_parse::{
    layer_from_check_message, looks_like_search_url, parse_debug_text, DebugParse,
};
pub use engine::{diagnose_from_debug, diagnose_gate_skip, gate_blocks_diagnose};
pub use port::ParseDiagnosePort;
