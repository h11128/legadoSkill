//! Integration tests for source_spine oneshot / ApplyService (§14.5).

use source_ports::SourceRepository;
use source_spine::fakes::{BusyChannel, FixedClock, IdleChannel, MemLedger, MemRepo, MemVerify};
use source_spine::{
    emit_report_json, idempotency_key, run_repair_oneshot, ApplyService, GateInput,
    IdempotencyStore, MemoryIdempotency, NoopRepairPlugin, PlanOrPlugin, RepairContext,
    RepairPorts, REPORT_JSON_PREFIX,
};
use source_types::{
    BookSource, Capability, GateAction, GateResult, PatchOp, PatchPlan, ReportStatus, SiteFamily,
    SourceKey, Url,
};
use serde_json::json;
use std::cell::RefCell;

fn sample_source(url: &str) -> BookSource {
    BookSource::new(json!({
        "bookSourceUrl": url,
        "bookSourceName": "demo",
        "searchUrl": "/old"
    }))
}

fn sample_plan(url: &str) -> PatchPlan {
    PatchPlan::new(
        Capability::Repair,
        SiteFamily::new(SiteFamily::XUNSEARCH_PID),
        Url::new(url).unwrap(),
        vec![
            PatchOp::set("searchUrl", json!("/search.php?q={{key}}")),
            PatchOp::set("ruleSearch.bookList", json!(".result-item")),
        ],
        "xunsearch list selector drifted",
    )
}

fn ctx_for(url: &str, source: BookSource) -> RepairContext {
    RepairContext::builder(SourceKey::new(url), source).build()
}

#[test]
fn apply_success_claims_fixed_only_with_verify() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let clock = FixedClock::default();
    let ctx = ctx_for(url, source);
    let plan = sample_plan(url);

    let out = ApplyService::apply(
        &ctx, &plan, &repo, &verify, &ledger, &clock, None,
    )
    .unwrap();

    assert_eq!(out.report.status, ReportStatus::Fixed);
    assert!(out.report.verify.as_ref().unwrap().success);
    assert!(out.report_line.starts_with(REPORT_JSON_PREFIX));
    assert_eq!(out.exit_code, 0);
    assert_eq!(*verify.calls.borrow(), 1);
    let saved = repo.get(&SourceKey::new(url)).unwrap();
    assert_eq!(
        saved.as_value()["searchUrl"],
        json!("/search.php?q={{key}}")
    );
}

#[test]
fn verify_fail_after_save_does_not_claim_fixed() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify {
        success: RefCell::new(false),
        message: RefCell::new("搜索失效".into()),
        calls: RefCell::new(0),
    };
    let ledger = MemLedger::default();
    let clock = FixedClock::default();
    let ctx = ctx_for(url, source);
    let plan = sample_plan(url);

    let out = ApplyService::apply(
        &ctx, &plan, &repo, &verify, &ledger, &clock, None,
    )
    .unwrap();

    assert_eq!(out.report.status, ReportStatus::Failed);
    assert!(out.verify_failed_after_save);
    assert!(out.saved);
    // Patched source kept (default no rollback).
    let saved = repo.get(&SourceKey::new(url)).unwrap();
    assert_eq!(
        saved.as_value()["ruleSearch"]["bookList"],
        json!(".result-item")
    );
    let notes: Vec<_> = ledger
        .rows
        .borrow()
        .iter()
        .filter_map(|r| r.note.clone())
        .collect();
    assert!(notes.iter().any(|n| n.contains("verify_failed_after_save")));
}

#[test]
fn empty_ops_validate_fails_without_save() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let clock = FixedClock::default();
    let ctx = ctx_for(url, source);
    let mut plan = sample_plan(url);
    plan.ops.clear();

    let out = ApplyService::apply(
        &ctx, &plan, &repo, &verify, &ledger, &clock, None,
    )
    .unwrap();
    assert_eq!(out.report.status, ReportStatus::Failed);
    assert!(!out.saved);
    assert_eq!(*verify.calls.borrow(), 0);
}

#[test]
fn dry_run_skips_save_and_verify() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let clock = FixedClock::default();
    let ctx = RepairContext::builder(SourceKey::new(url), source)
        .dry_run(true)
        .build();
    let plan = sample_plan(url);

    let out = ApplyService::apply(
        &ctx, &plan, &repo, &verify, &ledger, &clock, None,
    )
    .unwrap();
    assert!(out.dry_run);
    assert!(!out.saved);
    assert_eq!(*verify.calls.borrow(), 0);
    assert_eq!(
        repo.get(&SourceKey::new(url)).unwrap().as_value()["searchUrl"],
        json!("/old")
    );
}

#[test]
fn channel_busy_maps_to_exit_5() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = BusyChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = ctx_for(url, source);
    let gate = GateResult::new(Url::new(url).unwrap(), GateAction::Verify, "passed");
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plan(sample_plan(url)),
        GateInput::Injected(gate),
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.exit_code, 5);
    assert_eq!(out.report.status, ReportStatus::Failed);
    assert!(out.report.message.contains("channel_busy"));
}

#[test]
fn gate_skip_emits_skipped_report() {
    let url = "https://www.qidian.com/book/1";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = ctx_for(url, source);
    let gate = GateResult::new(Url::new(url).unwrap(), GateAction::Skip, "waf_official");
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&NoopRepairPlugin),
        GateInput::Injected(gate),
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.report.status, ReportStatus::Skipped);
    assert_eq!(out.exit_code, 0);
    assert_eq!(*verify.calls.borrow(), 0);
    assert!(out.report_line.starts_with(REPORT_JSON_PREFIX));
}

#[test]
fn gate_migrate_early_exits_without_apply() {
    let url = "https://old.example-novel.com/";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = ctx_for(url, source);
    let mut gate = GateResult::new(
        Url::new(url).unwrap(),
        GateAction::Migrate,
        "l2_host_redirect",
    );
    gate.migrate_to = Some(source_types::MigrateTarget::Host(
        source_types::HostKey::new("new.example-novel.com"),
    ));
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&NoopRepairPlugin),
        GateInput::Injected(gate),
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.report.status, ReportStatus::Migrated);
    assert_eq!(out.report.capability, Capability::Migrate);
    assert_eq!(
        out.report.migrate_to.as_deref(),
        Some("new.example-novel.com")
    );
    assert_eq!(out.exit_code, 0);
    assert_eq!(*verify.calls.borrow(), 0);
    assert!(out.apply.is_none());
}

#[test]
fn noop_plugin_returns_skipped_unrepairable() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = ctx_for(url, source);
    let gate = GateResult::new(Url::new(url).unwrap(), GateAction::Verify, "passed_l0");
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&NoopRepairPlugin),
        GateInput::Injected(gate),
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.report.status, ReportStatus::Skipped);
    assert!(out.report.message.contains("noop plugin"));
}

#[test]
fn idempotency_short_circuit_skips_repatch() {
    let url = "https://www.example-novel.com";
    let source = sample_source(url);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let clock = FixedClock::default();
    let ctx = ctx_for(url, source);
    let plan = sample_plan(url);
    let mut idem = MemoryIdempotency::default();
    let key = idempotency_key(&SourceKey::new(url), &plan.ops).unwrap();
    idem.remember_ok(&key);

    let out = ApplyService::apply(
        &ctx, &plan, &repo, &verify, &ledger, &clock, Some(&mut idem),
    )
    .unwrap();
    assert_eq!(out.report.status, ReportStatus::Fixed);
    assert!(!out.saved);
    assert_eq!(*verify.calls.borrow(), 0);
    assert!(out.report.message.contains("idempotent"));
}

#[test]
fn emit_report_json_prefix() {
    let report = source_types::ReportJson::new(
        Capability::Repair,
        source_types::Mode::Oneshot,
        Url::new("https://example.com/").unwrap(),
        ReportStatus::Skipped,
        "x",
    );
    let line = emit_report_json(&report).unwrap();
    assert!(line.starts_with("REPORT_JSON:{"));
}

#[test]
fn registry_plugin_with_form_html_proposes_search_url() {
    use source_adapters::{AdapterRegistry, RegistryRepairPlugin};

    let url = "https://m.example-form.com/";
    let html = br#"<form action="/s.php"><input name="q"/></form>"#;
    let source = sample_source(url);
    let reg = AdapterRegistry::with_seed_families();
    let plugin = RegistryRepairPlugin(&reg);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = RepairContext::builder(SourceKey::new(url), source)
        .dry_run(true)
        .no_verify(true)
        .insert_html(Url::new(url).unwrap(), html.to_vec())
        .build();
    let gate = GateResult::new(Url::new(url).unwrap(), GateAction::Verify, "passed");
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&plugin),
        GateInput::Injected(gate),
        Some(&reg),
        None,
    )
    .unwrap();
    let apply = out.apply.expect("dry apply");
    assert!(apply.dry_run);
    assert_eq!(out.report.status, ReportStatus::Skipped);
    assert!(out.report.message.contains("dry_run"));
    assert_eq!(
        out.report.family.as_ref().map(|f| f.as_str()),
        Some(SiteFamily::GENERIC_FORM)
    );
    assert!(
        out.report
            .ops_summary
            .as_ref()
            .map(|ops| ops.iter().any(|o| o.contains("searchUrl")))
            .unwrap_or(false),
        "ops_summary={:?}",
        out.report.ops_summary
    );
}

#[test]
fn registry_unknown_identify_without_html_is_unrepairable() {
    use source_adapters::{AdapterRegistry, RegistryRepairPlugin};

    let url = "https://m.example-blank.com/";
    let source = sample_source(url);
    let reg = AdapterRegistry::with_seed_families();
    let plugin = RegistryRepairPlugin(&reg);
    let repo = MemRepo::with_source(source.clone());
    let verify = MemVerify::default();
    let ledger = MemLedger::default();
    let channel = IdleChannel;
    let clock = FixedClock::default();
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let ctx = RepairContext::builder(SourceKey::new(url), source)
        .dry_run(true)
        .no_verify(true)
        .build();
    let gate = GateResult::new(Url::new(url).unwrap(), GateAction::Verify, "passed");
    let out = run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&plugin),
        GateInput::Injected(gate),
        Some(&reg),
        None,
    )
    .unwrap();
    assert!(out.apply.is_none());
    assert_eq!(out.report.status, ReportStatus::Skipped);
    assert!(
        out.report.message.contains("no repair plugin")
            || out.report.message.contains("Unknown"),
        "msg={}",
        out.report.message
    );
    assert_eq!(*verify.calls.borrow(), 0);
}
