//! Search-result page scoring (deterministic; golden-testable).

mod score;

pub use score::{score_search_html, ProbeScore};
