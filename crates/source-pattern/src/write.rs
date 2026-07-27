//! Optional PatternCluster JSON writer (tests use tempfile — not production assets/).

use std::fs;
use std::path::{Path, PathBuf};

use source_types::PatternCluster;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Write `<dir>/<family>.json`. Caller supplies dir (tempfile in tests).
pub fn write_cluster_json(cluster: &PatternCluster, dir: &Path) -> Result<PathBuf, WriteError> {
    fs::create_dir_all(dir)?;
    let name = format!("{}.json", cluster.family.as_str());
    let path = dir.join(name);
    let body = serde_json::to_vec_pretty(cluster)?;
    fs::write(&path, body)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::{Fingerprint, PartialBookSource, SiteFamily, Url};

    #[test]
    fn writes_under_temp_templates_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let templates = tmp.path().join("assets").join("templates");
        let cluster = PatternCluster::new(
            SiteFamily::new(SiteFamily::XUNSEARCH_PID),
            3,
            Fingerprint {
                signals: vec!["search:xunsearch_q".into()],
                structural_hash: "abc123def4567890".into(),
                confidence: 0.9,
            },
            PartialBookSource::new(json!({"searchUrl": "/search.php?q={{key}}"})),
            vec![Url::new("https://a.example").unwrap()],
            "2026-07-27T00:00:00Z",
        );
        let path = write_cluster_json(&cluster, &templates).unwrap();
        assert!(path.ends_with("XunsearchPid.json"));
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("XunsearchPid"));
        assert!(raw.contains("structural_hash"));
    }
}
