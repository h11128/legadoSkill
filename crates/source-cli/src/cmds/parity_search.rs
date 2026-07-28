//! Search-layer golden fixtures — parity with `parity_search_suite.py`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use source_mcp::repo_root;
use source_probe::probe_search;

pub fn run_search_parity_suite() -> Value {
    let root = match repo_root() {
        Ok(r) => r,
        Err(e) => {
            return json!({
                "suite": "search-parity",
                "ok": false,
                "detail": format!("repo_root: {e}"),
            });
        }
    };
    let fixtures = root.join("fixtures/expected/probe");
    if !fixtures.is_dir() {
        return json!({
            "suite": "search-parity",
            "ok": true,
            "detail": "no probe fixtures",
            "checked": 0,
        });
    }

    let mut failures = Vec::new();
    let mut checked = 0usize;
    for path in sorted_json_fixtures(&fixtures) {
        match check_fixture(&path) {
            Ok(()) => checked += 1,
            Err(msg) => failures.push(msg),
        }
    }
    let ok = failures.is_empty();
    json!({
        "suite": "search-parity",
        "ok": ok,
        "checked": checked,
        "detail": if failures.is_empty() {
            format!("checked {checked}")
        } else {
            format!("checked {checked}; {}", failures.iter().take(5).cloned().collect::<Vec<_>>().join("; "))
        },
    })
}

fn sorted_json_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
}

fn check_fixture(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let spec: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let html = spec.get("html").and_then(|v| v.as_str()).unwrap_or("");
    let base = spec
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://example.com/");
    let out = probe_search(html, base, "我的");
    let best = out
        .best
        .as_ref()
        .map(|b| b.search_url.as_str())
        .unwrap_or("");
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    for needle in spec
        .get("expected_best_contains")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let s = needle.as_str().unwrap_or("");
        if !best.contains(s) {
            return Err(format!("{name}: expected {s:?} in {best:?}"));
        }
    }
    for bad in spec
        .get("forbidden_best_contains")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let s = bad.as_str().unwrap_or("");
        if !s.is_empty() && best.contains(s) {
            return Err(format!("{name}: forbidden {s:?} in {best:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_parity_fixtures_pass() {
        let v = run_search_parity_suite();
        assert_eq!(v.get("ok"), Some(&json!(true)), "{v}");
    }
}
