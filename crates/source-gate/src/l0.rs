//! L0 denylist — parity with `scripts/repair_prefilter.py` `match_l0` / L0 branch.

use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use source_types::{GateAction, GateResult, L0Hit, Url};
use std::fs;
use std::path::Path;

use crate::{GateError, Result};

#[derive(Debug, Clone)]
pub struct SkipRule {
    pub id: String,
    pub pattern: String,
    pub action: GateAction,
    pub reason: String,
    re: Regex,
}

#[derive(Debug, Deserialize)]
struct RulesFile {
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    id: Option<String>,
    pattern: Option<String>,
    action: Option<String>,
    reason: Option<String>,
}

/// Load `config/verify_skip_rules.json` (or any same-shaped file).
pub fn load_rules(path: impl AsRef<Path>) -> Result<Vec<SkipRule>> {
    let text = fs::read_to_string(path.as_ref())?;
    let file: RulesFile = serde_json::from_str(&text)?;
    let mut out = Vec::with_capacity(file.rules.len());
    for raw in file.rules {
        let pat = raw.pattern.unwrap_or_default();
        if pat.is_empty() {
            continue;
        }
        let id = raw.id.clone().unwrap_or_default();
        let action = GateAction::parse(raw.action.as_deref().unwrap_or("skip"));
        let reason = raw
            .reason
            .clone()
            .or(raw.id.clone())
            .unwrap_or_else(|| "denylist".into());
        let re = RegexBuilder::new(&pat)
            .case_insensitive(true)
            .build()
            .map_err(|e| GateError::Msg(format!("bad pattern {id:?}: {e}")))?;
        out.push(SkipRule {
            id,
            pattern: pat,
            action,
            reason,
            re,
        });
    }
    Ok(out)
}

/// First matching L0 rule, or `None` (same order as Python).
pub fn match_l0(url: &str, rules: &[SkipRule]) -> Option<L0Hit> {
    for rule in rules {
        if rule.re.is_match(url) {
            return Some(L0Hit {
                rule_id: rule.id.clone(),
                action: rule.action,
                reason: rule.reason.clone(),
            });
        }
    }
    None
}

fn to_url(raw: &str) -> Url {
    // Python matches on raw bookSourceUrl strings; accept http(s) or wrap.
    match Url::new(raw) {
        Ok(u) => u,
        Err(_) => {
            let padded = if raw.contains("://") {
                raw.to_string()
            } else {
                format!("http://{raw}")
            };
            Url::new(padded.trim())
                .unwrap_or_else(|_| Url::new("http://invalid.invalid/").expect("fallback url"))
        }
    }
}

/// L0 classify: hit → `GateResult` with `verify: false`; miss → `passed_l0`.
pub fn classify_l0(url: &str, rules: &[SkipRule]) -> GateResult {
    let typed = to_url(url);
    match match_l0(url, rules) {
        Some(hit) => GateResult::l0_deny(typed, hit),
        None => GateResult::passed_l0(typed),
    }
}
