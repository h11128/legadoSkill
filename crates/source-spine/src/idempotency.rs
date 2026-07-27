//! Idempotency key: sha256(source_key + canonical_ops_json) (§14.5).

use sha2::{Digest, Sha256};
use source_types::{PatchOp, SourceKey};

use crate::error::SpineError;

/// Hex-encoded SHA-256 over `source_key` + serde JSON of `ops`.
///
/// Same key + ops → same digest; ApplyService may short-circuit when a prior
/// run already verified success for this key (optional).
/// Serialization failure is an error — never collapse to `"[]"` (fake short-circuit).
pub fn idempotency_key(source_key: &SourceKey, ops: &[PatchOp]) -> Result<String, SpineError> {
    let ops_json =
        serde_json::to_string(ops).map_err(|e| SpineError::Internal(format!("ops json: {e}")))?;
    let mut hasher = Sha256::new();
    hasher.update(source_key.as_str().as_bytes());
    hasher.update(ops_json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::PatchOp;

    #[test]
    fn stable_for_same_ops() {
        let key = SourceKey::new("https://example.com/");
        let ops = vec![PatchOp::set("searchUrl", json!("/s?q={{key}}"))];
        let a = idempotency_key(&key, &ops).unwrap();
        let b = idempotency_key(&key, &ops).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn changes_when_ops_differ() {
        let key = SourceKey::new("https://example.com/");
        let a = idempotency_key(&key, &[PatchOp::set("searchUrl", json!("a"))]).unwrap();
        let b = idempotency_key(&key, &[PatchOp::set("searchUrl", json!("b"))]).unwrap();
        assert_ne!(a, b);
    }
}
