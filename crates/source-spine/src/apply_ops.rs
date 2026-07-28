//! Patch op application helpers for ApplyService.

use serde_json::Value;
use source_patch::apply_ops;
use source_types::{BookSource, PatchOp, PatchOpKind};

use crate::error::SpineError;

pub(crate) fn apply_ops_to_source(
    before: &BookSource,
    ops: &[PatchOp],
) -> Result<BookSource, SpineError> {
    let mut root = before.clone().into_value();
    if !root.is_object() {
        return Err(SpineError::Contract(
            "BookSource root must be object".into(),
        ));
    }
    let mut pairs: Vec<(String, Option<Value>)> = Vec::with_capacity(ops.len());
    for op in ops {
        match op.op {
            PatchOpKind::Set => {
                let path = op
                    .path
                    .as_ref()
                    .ok_or_else(|| SpineError::Contract("set op missing path".into()))?;
                let value = op
                    .value
                    .clone()
                    .ok_or_else(|| SpineError::Contract("set op missing value".into()))?;
                pairs.push((path.as_str().to_string(), Some(value)));
            }
            PatchOpKind::Delete => {
                let path = op
                    .path
                    .as_ref()
                    .ok_or_else(|| SpineError::Contract("delete op missing path".into()))?;
                pairs.push((path.as_str().to_string(), None));
            }
            PatchOpKind::MigrateHost => {
                let to = op
                    .to_url
                    .as_ref()
                    .ok_or_else(|| SpineError::Contract("migrate_host missing to_url".into()))?;
                pairs.push((
                    "bookSourceUrl".to_string(),
                    Some(Value::String(to.as_str().to_string())),
                ));
            }
            other => {
                return Err(SpineError::Contract(format!(
                    "unsupported patch op in ApplyService: {other:?}"
                )));
            }
        }
    }
    apply_ops(&mut root, &pairs)?;
    Ok(BookSource::new(root))
}

pub(crate) fn ops_summary(ops: &[PatchOp]) -> Vec<String> {
    ops.iter()
        .map(|op| {
            let path = op.path.as_ref().map(|p| p.as_str()).unwrap_or("");
            match op.op {
                PatchOpKind::Set => format!("set {path}"),
                PatchOpKind::Delete => format!("delete {path}"),
                PatchOpKind::MigrateHost => "migrate_host".into(),
                PatchOpKind::MergeInto => "merge_into".into(),
                PatchOpKind::DeleteSource => "delete_source".into(),
                PatchOpKind::DisableSource => "disable_source".into(),
            }
        })
        .collect()
}
