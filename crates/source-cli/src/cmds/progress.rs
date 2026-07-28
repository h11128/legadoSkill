//! Progress status/next — L2-gated candidates from phone index / RT queue.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{json, Value};
use source_gate::{classify_one_l0, load_rules, SkipRule};

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

pub struct ProgressArgs {
    pub cmd: String, // status | next
    pub index: Option<PathBuf>,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
}

fn default_index() -> PathBuf {
    for p in [
        PathBuf::from("temp/full_fix/phone_source_index.json"),
        PathBuf::from("../temp/full_fix/phone_source_index.json"),
    ] {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("temp/full_fix/phone_source_index.json")
}

fn default_queue() -> Option<PathBuf> {
    for p in [
        PathBuf::from("temp/full_fix/queues/repair_serial100_queue.json"),
        PathBuf::from("../temp/full_fix/queues/repair_serial100_queue.json"),
    ] {
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn default_rules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json")
}

fn load_json(path: &PathBuf) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn default_ledger() -> Option<PathBuf> {
    for p in [
        PathBuf::from("temp/full_fix/repair_session_ledger.jsonl"),
        PathBuf::from("../temp/full_fix/repair_session_ledger.jsonl"),
    ] {
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn norm_url(u: &str) -> String {
    u.trim().trim_end_matches('/').to_string()
}

fn is_fixed_row(step: &str, result: &str) -> bool {
    step == "check"
        && (result.contains(source_types::LEDGER_VERIFY_OK)
            || result.starts_with("fixed:")
            || result.starts_with("fixed "))
}

/// A finished attempt: skip / disable / repurposed / explicit give-up.
/// `fail:` belongs here — without it a URL the agent gave up on is offered again
/// by the next `progress next`.
fn is_attempt_closed(step: &str, result: &str) -> bool {
    step == "skip"
        || result.starts_with("skip:")
        || result.starts_with("repurposed:")
        || result.starts_with("disable:")
        || result.starts_with("fail:")
        || (step == "check" && (result.starts_with("disable") || result.starts_with("fail")))
}

/// Incomplete work (no patch tried, transient verify failure) stays pickable
/// unless a hard marker such as dead host / disable overrides it.
fn is_retryable_reason(reason: &str) -> bool {
    let soft = reason.contains("no_patch")
        || reason.contains("搜索")
        || reason.contains("verify_fail")
        || reason.contains("校验失败");
    let hard = reason.starts_with("disable")
        || reason.starts_with("skip:l2")
        || reason.starts_with("skip:dead")
        || reason.starts_with("skip:wall")
        || reason.starts_with("skip:park")
        || reason.starts_with("repurposed:")
        || reason.starts_with("skip:jieqi")
        || reason.starts_with("skip:biquge")
        || reason.contains("search_empty")
        || reason.contains("search_index_empty")
        || reason.contains("http_dead")
        || reason.contains("domain_repurposed");
    soft && !hard
}

/// Hard-done URLs only (fixed / disable / dead skip / gave-up fail).
fn ledger_blocked() -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let Some(path) = default_ledger() else {
        return HashSet::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    blocked_from_lines(raw.lines())
}

fn blocked_from_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> std::collections::HashSet<String> {
    use std::collections::{HashMap, HashSet};
    let mut fixed: HashSet<String> = HashSet::new();
    let mut forced: HashSet<String> = HashSet::new();
    let mut last_closed: HashMap<String, String> = HashMap::new();
    for line in lines {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let u = norm_url(row.get("url").and_then(|v| v.as_str()).unwrap_or(""));
        if u.is_empty() {
            continue;
        }
        let result = row.get("result").and_then(|v| v.as_str()).unwrap_or("");
        let step = row.get("step").and_then(|v| v.as_str()).unwrap_or("");
        if is_fixed_row(step, result) {
            fixed.insert(u.clone());
        }
        // `final: true` is written by repair_retro for a terminal fail/skip and
        // is not downgraded by soft wording in the reason text.
        if row.get("final").and_then(|v| v.as_bool()).unwrap_or(false) {
            forced.insert(u.clone());
        }
        if is_attempt_closed(step, result) {
            last_closed.insert(u, result.to_string());
        }
    }
    let mut hard = HashSet::new();
    for (u, reason) in last_closed {
        if fixed.contains(&u) || is_retryable_reason(&reason) {
            continue;
        }
        hard.insert(u);
    }
    hard.extend(forced);
    hard.extend(fixed);
    hard
}

/// Prefer RT queue order; fall back to phone-index search-fail URLs (alpha).
fn candidate_urls(index: &Value, queue: Option<&Value>) -> Vec<(String, Value)> {
    let blocked = ledger_blocked();
    let mut out = Vec::new();
    if let Some(q) = queue {
        if let Some(arr) = q
            .get("items")
            .or_else(|| q.get("urls"))
            .and_then(|v| v.as_array())
        {
            for row in arr {
                let url = row
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if url.starts_with("http") && !blocked.contains(&norm_url(&url)) {
                    out.push((url, row.clone()));
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
    }
    if let Some(by) = index.get("by_url").and_then(|v| v.as_object()) {
        for (url, meta) in by {
            let g = meta.get("group").and_then(|v| v.as_str()).unwrap_or("");
            if !g.contains("搜索失效") {
                continue;
            }
            if meta.get("enabled") == Some(&json!(false)) {
                continue;
            }
            if !url.starts_with("http") {
                continue;
            }
            if blocked.contains(&norm_url(url)) {
                continue;
            }
            out.push((url.clone(), meta.clone()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn gate_one(url: &str, rules: &[SkipRule], l0_only: bool) -> Value {
    let g = if l0_only {
        classify_one_l0(url, rules)
    } else {
        #[cfg(feature = "gate_full")]
        {
            classify_one(
                url,
                rules,
                &ClassifyOpts {
                    tcp_timeout_s: 1.5,
                    l2_timeout_s: 4.0,
                },
            )
        }
        #[cfg(not(feature = "gate_full"))]
        {
            classify_one_l0(url, rules)
        }
    };
    serde_json::to_value(&g).unwrap_or(json!({}))
}

fn closeout_script_root() -> Option<PathBuf> {
    for base in [PathBuf::from("."), PathBuf::from("..")] {
        let script = base.join("scripts/repair_closeout_check.py");
        if script.is_file() {
            return Some(base);
        }
    }
    None
}

fn ensure_closeout_ready() -> Result<(), String> {
    let Some(root) = closeout_script_root() else {
        return Ok(());
    };
    let script = root.join("scripts/repair_closeout_check.py");
    let py = if cfg!(windows) { "python" } else { "python3" };
    let out = std::process::Command::new(py)
        .current_dir(&root)
        .arg(&script)
        .arg("pending")
        .output()
        .map_err(|e| format!("close-out pending exec: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(format!("close-out pending blocked:\n{stdout}{stderr}"))
}

pub fn run_progress(args: ProgressArgs) -> ExitCode {
    if args.l0_only {
        eprintln!(
            "progress: warn: --l0-only skips L2 dead/wall; do not use for live repair pick"
        );
    }
    let index_path = args.index.unwrap_or_else(default_index);
    let index = match load_json(&index_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("progress: index: {e}");
            return ExitCode::from(4);
        }
    };
    if args.cmd == "status" {
        let queue = default_queue().and_then(|p| load_json(&p).ok());
        let cands = candidate_urls(&index, queue.as_ref());
        println!(
            "{}",
            json!({
                "index": index_path.to_string_lossy(),
                "search_fail_candidates": cands.len(),
                "from_queue": queue.is_some(),
                "total": index.get("total").cloned().unwrap_or(json!(null)),
            })
        );
        return ExitCode::SUCCESS;
    }
    if let Err(msg) = ensure_closeout_ready() {
        eprintln!("progress: {msg}");
        return ExitCode::from(3);
    }
    let queue = default_queue().and_then(|p| load_json(&p).ok());
    let rules_path = args.rules.unwrap_or_else(default_rules);
    let rules = match load_rules(&rules_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("progress: rules: {e}");
            return ExitCode::from(4);
        }
    };
    let cands = candidate_urls(&index, queue.as_ref());
    for (url, meta) in cands.iter().take(40) {
        let g = gate_one(url, &rules, args.l0_only);
        let action = g.get("action").and_then(|a| a.as_str()).unwrap_or("");
        if action == "verify" {
            println!(
                "{}",
                json!({
                    "next": {
                        "url": url,
                        "name": meta.get("name"),
                        "group": meta.get("group"),
                        "respondTime": meta.get("respondTime"),
                        "l2_gate": g,
                        "status": "candidate",
                    }
                })
            );
            return ExitCode::SUCCESS;
        }
    }
    println!("{}", json!({"next": null, "hint": "no verify-pass candidate in first 40"}));
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(url: &str, step: &str, result: &str) -> String {
        json!({"url": url, "step": step, "result": result}).to_string()
    }

    #[test]
    fn gave_up_fail_blocks_repick() {
        let lines = vec![row(
            "https://ac.qq.com",
            "check",
            "fail:content_redirect_encrypted_chapter",
        )];
        let blocked = blocked_from_lines(lines.iter().map(String::as_str));
        assert!(blocked.contains("https://ac.qq.com"));
    }

    #[test]
    fn transient_fail_stays_pickable() {
        let lines = vec![row("https://a.test", "check", "fail:verify_fail")];
        let blocked = blocked_from_lines(lines.iter().map(String::as_str));
        assert!(blocked.is_empty());
    }

    #[test]
    fn later_fix_overrides_earlier_fail() {
        let lines = vec![
            row("https://b.test", "check", "fail:toc_empty"),
            row("https://b.test", "check", "校验成功"),
        ];
        let blocked = blocked_from_lines(lines.iter().map(String::as_str));
        assert!(blocked.contains("https://b.test"));
    }

    #[test]
    fn soft_skip_and_hard_skip_differ() {
        let lines = vec![
            row("https://soft.test", "skip", "skip:no_patch"),
            row("https://hard.test", "skip", "skip:dead_host"),
        ];
        let blocked = blocked_from_lines(lines.iter().map(String::as_str));
        assert!(!blocked.contains("https://soft.test"));
        assert!(blocked.contains("https://hard.test"));
    }

    #[test]
    fn final_flag_beats_soft_wording() {
        let line =
            json!({"url": "https://d.test", "step": "check", "result": "fail:校验失败", "final": true})
                .to_string();
        let blocked = blocked_from_lines(std::iter::once(line.as_str()));
        assert!(blocked.contains("https://d.test"));
    }

    #[test]
    fn trailing_slash_is_normalized() {
        let lines = vec![row("https://c.test/", "check", "fail:dead")];
        let blocked = blocked_from_lines(lines.iter().map(String::as_str));
        assert!(blocked.contains("https://c.test"));
    }
}
