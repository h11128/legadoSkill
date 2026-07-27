//! Identify SiteFamily from BookSource + HTML via FingerprintRule weights (§3.4 / §10.5).

mod match_rule;
mod score;

pub use match_rule::rule_matches;
pub use score::{identify, score_family, FamilyRules};
