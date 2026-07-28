//! Parse CSS/@js selector strings for common smells.

use regex::Regex;
use serde_json::{json, Value};

pub fn analyze_rule(rule: &str) -> Value {
    let mut tips = Vec::new();
    if rule.contains("@js:") || rule.contains("@Js:") {
        tips.push("contains @js block");
    }
    if rule.contains("{{") {
        tips.push("uses template placeholders");
    }
    if rule.contains("||") {
        tips.push("fallback chain (||)");
    }
    let re = Regex::new(r"@[a-zA-Z]+").unwrap();
    let attrs: Vec<_> = re.find_iter(rule).map(|m| m.as_str()).collect();
    json!({
        "rule": rule,
        "length": rule.len(),
        "attrs": attrs,
        "tips": tips,
    })
}
