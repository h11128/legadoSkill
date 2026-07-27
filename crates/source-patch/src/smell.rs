use serde_json::Value;

/// Wave rule: ops that only touch concurrentRate are not a real fix.
pub fn is_rate_only_ops(paths: &[&str]) -> bool {
    !paths.is_empty() && paths.iter().all(|p| *p == "concurrentRate")
}

/// Hint helper: exploreUrl present but empty-ish.
pub fn strip_dead_explore_hint(source: &Value) -> bool {
    match source.get("exploreUrl") {
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(Value::Null) | None => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_only() {
        assert!(is_rate_only_ops(&["concurrentRate"]));
        assert!(!is_rate_only_ops(&["concurrentRate", "searchUrl"]));
    }
}
