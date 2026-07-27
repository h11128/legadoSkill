//! Scheme normalize + smell/auto prep before oneshot diagnose.

use source_patch::{apply_auto_patches, apply_safe_rule_fixes, normalize_source_schemes};
use source_ports::SourceRepository;
use source_types::BookSource;

/// Apply scheme http:// fixes + safe smells. Saves when scheme fields changed.
/// Returns notes for logging.
pub fn prep_source_before_repair(
    source: &mut BookSource,
    repo: &dyn SourceRepository,
    dry_run: bool,
) -> Vec<String> {
    let mut notes = normalize_source_schemes(source);
    let smell = apply_safe_rule_fixes(source);
    let auto = apply_auto_patches(source);
    notes.extend(smell);
    notes.extend(auto);
    if !dry_run && notes.iter().any(|n| n.starts_with("scheme_http:")) {
        if let Err(e) = repo.save(source) {
            eprintln!("repair: scheme save warn: {e}");
        }
    }
    notes
}
