//! JSON Schema contract validators for repair wire formats (architecture §8).
//!
//! Schemas live under `config/repair_contracts/` at the legadoSkill repo root.
//! Resolve root via `LEGADO_SKILL_ROOT` or by walking up from this crate's
//! `CARGO_MANIFEST_DIR`.

/// Re-export shared schema version from `source_types`.
pub use source_types::SCHEMA_VERSION;

mod error;
mod registry;
mod root;
mod validate;

pub use error::ContractError;
pub use registry::{contracts_dir, reload, validate_named, SCHEMA_NAMES};
pub use root::repo_root;
pub use validate::{
    validate_diagnose, validate_gate, validate_identify, validate_ledger, validate_merge,
    validate_optimize, validate_patch, validate_pattern, validate_report, validate_verify,
};

#[cfg(test)]
mod fixture_tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn fixtures_root() -> PathBuf {
        repo_root().expect("repo root").join("fixtures/expected/contracts")
    }

    fn load_json(path: &PathBuf) -> Value {
        let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    }

    fn stem_to_validator(stem: &str) -> fn(&Value) -> Result<(), ContractError> {
        match stem {
            "report_json" => validate_report,
            "gate_result" => validate_gate,
            "diagnose_result" => validate_diagnose,
            "patch_plan" => validate_patch,
            "optimize_plan" => validate_optimize,
            "merge_plan" => validate_merge,
            "verify_result" => validate_verify,
            "ledger_row" => validate_ledger,
            "pattern_cluster" => validate_pattern,
            "identify_result" => validate_identify,
            other => panic!("unknown fixture contract stem: {other}"),
        }
    }

    #[test]
    fn valid_fixtures_pass() {
        let dir = fixtures_root().join("valid");
        let mut count = 0usize;
        for entry in fs::read_dir(&dir).expect("valid fixtures dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("stem")
                .to_string();
            // Allow names like `report_json_fixed.json` → report_json
            let contract = SCHEMA_NAMES
                .iter()
                .find(|n| stem == **n || stem.starts_with(&format!("{n}_")))
                .unwrap_or_else(|| panic!("no schema for fixture {stem}"));
            let value = load_json(&path);
            stem_to_validator(contract)(&value)
                .unwrap_or_else(|e| panic!("expected valid {}: {e}", path.display()));
            count += 1;
        }
        assert!(count >= 10, "expected ≥10 valid fixtures, got {count}");
    }

    #[test]
    fn invalid_fixtures_fail() {
        let dir = fixtures_root().join("invalid");
        let mut count = 0usize;
        for entry in fs::read_dir(&dir).expect("invalid fixtures dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("stem")
                .to_string();
            let contract = SCHEMA_NAMES
                .iter()
                .find(|n| stem == **n || stem.starts_with(&format!("{n}_")))
                .unwrap_or_else(|| panic!("no schema for fixture {stem}"));
            let value = load_json(&path);
            let err = stem_to_validator(contract)(&value)
                .expect_err(&format!("expected invalid {}", path.display()));
            assert!(
                matches!(err, ContractError::Invalid { .. }),
                "fixture {} wrong error: {err}",
                path.display()
            );
            count += 1;
        }
        assert!(count >= 3, "expected ≥3 invalid fixtures, got {count}");
    }

    #[test]
    fn anti_fake_fixed_without_verify() {
        let value = serde_json::json!({
            "schema_version": "1",
            "capability": "repair",
            "mode": "oneshot",
            "url": "https://example.com",
            "status": "fixed",
            "message": "claimed fixed without verify"
        });
        let err = validate_report(&value).expect_err("must reject fake fixed");
        assert!(matches!(err, ContractError::Invalid { .. }));
    }
}
