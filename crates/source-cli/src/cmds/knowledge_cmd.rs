//! `source-cli knowledge search` — docs/assets lookup.

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use source_mcp::repo_root;
use source_queue::search_knowledge;

pub enum KnowledgeCmd {
    Search {
        query: String,
        layer: String,
        root: Option<PathBuf>,
    },
}

pub fn run_knowledge(cmd: KnowledgeCmd) -> ExitCode {
    match cmd {
        KnowledgeCmd::Search { query, layer, root } => {
            let root = match root {
                Some(r) => r,
                None => match repo_root() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("knowledge: {e}");
                        return ExitCode::from(1);
                    }
                },
            };
            let hits = search_knowledge(&root, &query, &layer);
            println!(
                "{}",
                json!({
                    "query": query,
                    "layer": layer,
                    "hits": hits,
                    "n": hits.len(),
                })
            );
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn search_finds_doc() {
        let dir = TempDir::new().unwrap();
        let docs = dir.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("note.md"), "tocUrl and 目录 pagination\n").unwrap();
        let code = run_knowledge(KnowledgeCmd::Search {
            query: "tocUrl".into(),
            layer: "toc".into(),
            root: Some(dir.path().to_path_buf()),
        });
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
