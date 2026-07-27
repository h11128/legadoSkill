//! Repair orchestration spine: Gate → Identify → Apply → Verify → Ledger (§4.1, §14.5).

mod apply;
mod apply_ops;
mod apply_report;
mod context;
mod error;
mod idempotency;
mod oneshot;
mod outcome;
mod plugin;
mod report_emit;

pub mod fakes;

pub use apply::ApplyService;
pub use apply_report::report_validate_failure;
pub use context::{HtmlCache, RepairContext, RepairContextBuilder};
pub use error::SpineError;
pub use idempotency::idempotency_key;
pub use oneshot::{
    run_repair_oneshot, GateFn, GateInput, OneshotResult, PlanOrPlugin, RepairPorts,
};
pub use outcome::{ApplyOutcome, IdempotencyStore, MemoryIdempotency};
pub use plugin::{
    identify_stub, CreatePlugin, FamilyPlugin, NoopRepairPlugin, OptimizePlugin, RepairPlugin,
};
pub use report_emit::{emit_report_json, emit_report_line, REPORT_JSON_PREFIX};
