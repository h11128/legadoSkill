//! Apply dotted-path PatchOps onto BookSource JSON (App field SOT).

mod apply;
mod smell;

pub use apply::{apply_ops, ApplyError};
pub use smell::{is_rate_only_ops, strip_dead_explore_hint};
