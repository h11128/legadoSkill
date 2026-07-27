use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("empty path")]
    EmptyPath,
    #[error("cannot descend into non-object at {0}")]
    NotObject(String),
    #[error("missing value for set")]
    MissingValue,
}

/// Set or delete a dotted path on a JSON object (mutates in place).
pub fn apply_path(root: &mut Value, path: &str, value: Option<Value>) -> Result<(), ApplyError> {
    if path.is_empty() {
        return Err(ApplyError::EmptyPath);
    }
    let parts: Vec<&str> = path.split('.').collect();
    let (last, parents) = parts.split_last().unwrap();
    let mut cur = root;
    for (i, p) in parents.iter().enumerate() {
        if !cur.is_object() {
            return Err(ApplyError::NotObject(parents[..=i].join(".")));
        }
        let obj = cur.as_object_mut().unwrap();
        if !obj.contains_key(*p) {
            if value.is_none() {
                return Ok(()); // delete missing = no-op
            }
            obj.insert((*p).to_string(), json!({}));
        }
        cur = obj.get_mut(*p).unwrap();
    }
    let obj = cur
        .as_object_mut()
        .ok_or_else(|| ApplyError::NotObject(path.to_string()))?;
    match value {
        Some(v) => {
            obj.insert((*last).to_string(), v);
        }
        None => {
            obj.remove(*last);
        }
    }
    Ok(())
}

pub fn apply_ops(root: &mut Value, ops: &[(String, Option<Value>)]) -> Result<(), ApplyError> {
    for (path, val) in ops {
        apply_path(root, path, val.clone())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_nested() {
        let mut v = json!({"ruleSearch": {}});
        apply_path(&mut v, "ruleSearch.bookList", Some(json!(".item"))).unwrap();
        assert_eq!(v["ruleSearch"]["bookList"], ".item");
    }

    #[test]
    fn delete_field() {
        let mut v = json!({"exploreUrl": "x", "searchUrl": "y"});
        apply_path(&mut v, "exploreUrl", None).unwrap();
        assert!(v.get("exploreUrl").is_none());
    }
}
