//! Family adapters (ISP) + create/optimize/merge pure helpers (§14.3 / §11.4–11.5).

mod context;
mod create;
mod families;
mod form;
mod merge;
mod optimize;
mod registry;
mod traits;

pub use context::RepairContext;
pub use create::create_via_registry;
pub use families::{
    FictionListXchina, GenericForm, JieqiMobile, XunsearchPid, FICTION_LIST_XCHINA_RULES,
    GENERIC_FORM_RULES, JIEQI_MOBILE_RULES, XUNSEARCH_PID_RULES,
};
pub use merge::{
    respond_time_scores, rule_completeness, score_merge_candidate, MergeCandidateInput,
};
pub use optimize::{optimize_smells_plan, OptimizeSmellInput};
pub use registry::AdapterRegistry;
pub use traits::{CreatePlugin, FamilyPlugin, OptimizePlugin, RepairPlugin};
