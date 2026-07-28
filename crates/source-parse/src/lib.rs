//! Offline Legado rule/url analysis — port of debugger/engine.

mod analyze_rule;
mod analyze_url;

pub use analyze_rule::analyze_rule;
pub use analyze_url::analyze_url;
