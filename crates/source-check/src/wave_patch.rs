//! PC-side auto-patch before batch verify (Python `repair_wave.patch_one`).

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use source_mcp::{McpClient, McpSourceRepository};
use source_patch::{apply_auto_patches, apply_safe_rule_fixes, normalize_source_schemes};
use source_ports::SourceRepository;
use source_types::{PortError, SourceKey};

fn meaningful_changes(changes: &[String]) -> bool {
    changes.iter().any(|c| !c.contains("concurrentRate"))
}

pub fn patch_one(client: Arc<McpClient>, url: &str) -> Value {
    let t0 = Instant::now();
    let mut row = json!({ "url": url });
    let repo = McpSourceRepository::new(client);
    match patch_inner(&repo, url) {
        Ok(inner) => {
            for (k, v) in inner.as_object().into_iter().flatten() {
                row[k] = v.clone();
            }
        }
        Err(e) => {
            row["action"] = json!("error");
            row["error"] = json!(e.to_string());
            row["verify"] = json!(false);
        }
    }
    row["ms"] = json!(t0.elapsed().as_millis() as u64);
    row
}

fn patch_inner(repo: &McpSourceRepository, url: &str) -> Result<Value, PortError> {
    let key = SourceKey::new(url);
    let mut src = repo.get(&key)?;
    let name = src
        .as_value()
        .get("bookSourceName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut changes = normalize_source_schemes(&mut src);
    changes.extend(apply_safe_rule_fixes(&mut src));
    changes.extend(apply_auto_patches(&mut src));
    let meaningful = meaningful_changes(&changes);
    let action = if meaningful {
        "patched"
    } else if changes.is_empty() {
        "no_patch"
    } else {
        "rate_only"
    };
    if !changes.is_empty() {
        repo.save(&src)?;
    }
    Ok(json!({
        "name": name,
        "changes": changes,
        "meaningful": meaningful,
        "action": action,
        "verify": true,
        "save": !changes.is_empty(),
    }))
}
