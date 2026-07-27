use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use jsonschema::Validator;
use serde_json::Value;

use crate::error::ContractError;
use crate::root::repo_root;

/// Known contract schema file stems under `config/repair_contracts/`.
pub const SCHEMA_NAMES: &[&str] = &[
    "report_json",
    "gate_result",
    "diagnose_result",
    "patch_plan",
    "optimize_plan",
    "merge_plan",
    "verify_result",
    "ledger_row",
    "pattern_cluster",
    "identify_result",
];

type ValidatorMap = HashMap<&'static str, Validator>;

fn cache() -> &'static Mutex<Option<ValidatorMap>> {
    static CACHE: OnceLock<Mutex<Option<ValidatorMap>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Directory containing `*.schema.json` files.
pub fn contracts_dir() -> Result<PathBuf, ContractError> {
    Ok(repo_root()?.join("config/repair_contracts"))
}

fn schema_path(name: &str) -> Result<PathBuf, ContractError> {
    let path = contracts_dir()?.join(format!("{name}.schema.json"));
    if !path.is_file() {
        return Err(ContractError::SchemaMissing(path));
    }
    Ok(path)
}

fn load_schema_value(name: &str) -> Result<Value, ContractError> {
    let path = schema_path(name)?;
    let raw = fs::read_to_string(&path).map_err(|source| ContractError::SchemaIo {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| ContractError::SchemaParse { path, source })
}

fn compile_all() -> Result<ValidatorMap, ContractError> {
    let mut map = ValidatorMap::new();
    for &name in SCHEMA_NAMES {
        let schema = load_schema_value(name)?;
        let validator = jsonschema::options().build(&schema).map_err(|e| {
            ContractError::SchemaCompile {
                name: name.to_string(),
                message: e.to_string(),
            }
        })?;
        map.insert(name, validator);
    }
    Ok(map)
}

fn with_validators<R>(
    f: impl FnOnce(&ValidatorMap) -> Result<R, ContractError>,
) -> Result<R, ContractError> {
    let lock = cache();
    let mut guard = lock.lock().expect("contracts cache poisoned");
    if guard.is_none() {
        *guard = Some(compile_all()?);
    }
    f(guard.as_ref().expect("compiled"))
}

/// Validate `value` against the named schema (`report_json`, `gate_result`, …).
pub fn validate_named(name: &'static str, value: &Value) -> Result<(), ContractError> {
    with_validators(|map| {
        let Some(validator) = map.get(name) else {
            return Err(ContractError::invalid(name, "unknown contract schema name"));
        };
        match validator.validate(value) {
            Ok(()) => Ok(()),
            Err(err) => Err(ContractError::invalid(
                name,
                format!("{err} (at {})", err.instance_path),
            )),
        }
    })
}

/// Force-reload schemas from disk (tests / hot reload).
pub fn reload() -> Result<(), ContractError> {
    let compiled = compile_all()?;
    let lock = cache();
    let mut guard = lock.lock().expect("contracts cache poisoned");
    *guard = Some(compiled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_schemas_compile() {
        reload().expect("compile schemas");
        assert_eq!(SCHEMA_NAMES.len(), 10);
    }
}
