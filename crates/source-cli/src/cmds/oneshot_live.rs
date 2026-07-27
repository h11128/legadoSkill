//! Single-URL live repair body (channel + diagnose + spine).

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use source_adapters::{AdapterRegistry, RegistryRepairPlugin};
use source_diagnose::ParseDiagnosePort;
use source_gate::{classify_one_l0, load_rules};
use source_mcp::{
    FsChannelPort, JsonlLedgerPort, McpClient, McpEndpoint, McpSourceRepository, McpVerifyPort,
};
use source_patch::apply_safe_rule_fixes;
use source_ports::{ChannelPort, Clock, DiagnosePort, HtmlFetchPort, SourceRepository};
use source_spine::{
    run_repair_oneshot, DiagnoseInput, GateInput, PlanOrPlugin, RepairPorts,
};
use source_types::{FetchResult, HeaderMap, Layer, PortError, SourceKey, Url};

use super::search_plan::build_search_layer_plan;

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

fn default_rules_path() -> PathBuf {
    let candidates = [
        PathBuf::from("config/verify_skip_rules.json"),
        PathBuf::from("../config/verify_skip_rules.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json"),
    ];
    for p in candidates {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/verify_skip_rules.json")
}

struct UtcClock;
impl Clock for UtcClock {
    fn now_utc(&self) -> chrono::DateTime<Utc> {
        Utc::now()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

struct UreqFetch;
impl HtmlFetchPort for UreqFetch {
    fn fetch(&self, url: &Url, headers: &HeaderMap) -> Result<FetchResult, PortError> {
        let mut req = ureq::get(url.as_str());
        for (k, v) in headers.iter() {
            req = req.set(k, v);
        }
        let resp = req
            .call()
            .map_err(|e| PortError::Transient(format!("fetch {url}: {e}")))?;
        let status = resp.status();
        let mut body = Vec::new();
        resp.into_reader()
            .read_to_end(&mut body)
            .map_err(|e| PortError::Transient(format!("read body: {e}")))?;
        Ok(FetchResult::new(status, url.clone(), body))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn repair_one_url(
    url: &str,
    rules: Option<PathBuf>,
    l0_only: bool,
    tcp_timeout: f64,
    l2_timeout: f64,
    dry_run: bool,
    no_verify: bool,
    html: Option<PathBuf>,
    prefetch: bool,
    key: &str,
    skip_diagnose: bool,
) -> ExitCode {
    let path = rules.unwrap_or_else(default_rules_path);
    let rules = match load_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("repair: load rules: {e}");
            return ExitCode::from(4);
        }
    };

    let gate = if l0_only {
        classify_one_l0(url, &rules)
    } else {
        #[cfg(feature = "gate_full")]
        {
            let opts = ClassifyOpts {
                tcp_timeout_s: tcp_timeout,
                l2_timeout_s: l2_timeout,
            };
            classify_one(url, &rules, &opts)
        }
        #[cfg(not(feature = "gate_full"))]
        {
            let _ = (tcp_timeout, l2_timeout);
            classify_one_l0(url, &rules)
        }
    };

    let ep = match McpEndpoint::load_defaults() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("repair: mcp defaults: {e}");
            return ExitCode::from(4);
        }
    };
    let client = Arc::new(McpClient::new(ep).with_client_name("source_cli_repair"));
    if let Err(e) = client.ensure_session() {
        eprintln!("repair: mcp session: {e}");
        return ExitCode::from(2);
    }

    let channel = match FsChannelPort::from_repo() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("repair: channel: {e}");
            return ExitCode::from(4);
        }
    };
    if let Err(e) = channel.assert_idle_for_repair() {
        eprintln!("repair: {e}");
        return ExitCode::from(5);
    }
    let _guard = match channel.acquire_repair() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("repair: acquire: {e}");
            return ExitCode::from(5);
        }
    };

    let source_key = SourceKey::new(url.trim());
    let repo = McpSourceRepository::new(Arc::clone(&client));
    let mut source = match repo.get(&source_key) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("repair: get_source: {e}");
            return ExitCode::from(2);
        }
    };
    let _smell_changes = apply_safe_rule_fixes(&mut source);
    let source_for_diag = source.clone();

    let debug_text = if skip_diagnose {
        String::new()
    } else {
        match client.tools_call(
            "debug_source",
            serde_json::json!({"url": url, "key": key}),
        ) {
            Ok(v) => McpClient::extract_text(&v),
            Err(e) => {
                eprintln!("repair: debug_source warn: {e}");
                String::new()
            }
        }
    };

    let mut builder = source_spine::RepairContext::builder(source_key.clone(), source)
        .gate(gate.clone())
        .dry_run(dry_run)
        .no_verify(no_verify);

    if let Some(html_path) = &html {
        match std::fs::read(html_path) {
            Ok(body) => match Url::new(url.trim()) {
                Ok(u) => builder = builder.insert_html(u, body),
                Err(e) => {
                    eprintln!("repair: bad url for html inject: {e}");
                    return ExitCode::from(4);
                }
            },
            Err(e) => {
                eprintln!("repair: read html: {e}");
                return ExitCode::from(4);
            }
        }
    } else if prefetch {
        match Url::new(url.trim()) {
            Ok(u) => match UreqFetch.fetch(&u, &HeaderMap::new()) {
                Ok(fr) => builder = builder.insert_html(u, fr.body),
                Err(e) => eprintln!("repair: prefetch warn: {e}"),
            },
            Err(e) => {
                eprintln!("repair: bad url: {e}");
                return ExitCode::from(4);
            }
        }
    }

    let ctx = builder.build();
    let verify = McpVerifyPort::new(Arc::clone(&client));
    let ledger = match JsonlLedgerPort::from_defaults() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("repair: ledger: {e}");
            return ExitCode::from(4);
        }
    };
    let clock = UtcClock;
    let ports = RepairPorts {
        repo: &repo,
        verify: &verify,
        ledger: &ledger,
        channel: &channel,
        clock: &clock,
    };
    let reg = AdapterRegistry::with_seed_families();
    let plugin = RegistryRepairPlugin(&reg);
    let diag_port = ParseDiagnosePort;

    // Pre-parse layer so search can use probe+charset+bookList plan (Rust-only).
    let layer_preview = if !skip_diagnose && !debug_text.is_empty() {
        match Url::new(url.trim()) {
            Ok(u) => {
                let d = diag_port.diagnose(u, &source_for_diag, &debug_text, None);
                Some(d.layer)
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let mut search_plan = None;
    if layer_preview == Some(Layer::Search) {
        let home = ctx.html_text();
        if !home.trim().is_empty() {
            let fam = source_types::SiteFamily::new(source_types::SiteFamily::GENERIC_FORM);
            search_plan = build_search_layer_plan(url.trim(), &home, key, fam);
            if let Some(ref p) = search_plan {
                eprintln!(
                    "repair: search-layer plan ops={} rationale={}",
                    p.ops.len(),
                    p.rationale
                );
            }
        }
    }

    let diagnose = if skip_diagnose || debug_text.is_empty() {
        DiagnoseInput::None
    } else {
        DiagnoseInput::DebugText {
            text: &debug_text,
            fail_msg: None,
            port: &diag_port,
        }
    };

    let plan_or = match search_plan {
        Some(p) => PlanOrPlugin::Plan(p),
        None => PlanOrPlugin::Plugin(&plugin),
    };

    let result = match run_repair_oneshot(
        ctx,
        &ports,
        plan_or,
        GateInput::Injected(gate),
        Some(&reg),
        diagnose,
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("repair: spine: {e}");
            return ExitCode::from(2);
        }
    };
    println!("{}", result.report_line);
    ExitCode::from(result.exit_code as u8)
}
