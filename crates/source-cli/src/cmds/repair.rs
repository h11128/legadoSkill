//! Live oneshot repair via MCP ports + AdapterRegistry.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use source_adapters::{AdapterRegistry, RegistryRepairPlugin};
use source_gate::{classify_one_l0, load_rules};
use source_mcp::{
    FsChannelPort, JsonlLedgerPort, McpClient, McpEndpoint, McpSourceRepository, McpVerifyPort,
};
use source_ports::{ChannelPort, Clock, HtmlFetchPort, SourceRepository};
use source_spine::{run_repair_oneshot, GateInput, PlanOrPlugin, RepairPorts};
use source_types::{FetchResult, HeaderMap, PortError, SourceKey, Url};

#[cfg(feature = "gate_full")]
use source_gate::{classify_one, ClassifyOpts};

pub struct RepairArgs {
    pub url: String,
    pub rules: Option<PathBuf>,
    pub l0_only: bool,
    pub tcp_timeout: f64,
    pub l2_timeout: f64,
    pub dry_run: bool,
    pub no_verify: bool,
    pub html: Option<PathBuf>,
    pub prefetch: bool,
}

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

/// Minimal PC fetch for home HTML prefetch (ureq).
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

pub fn run_repair(args: RepairArgs) -> ExitCode {
    let path = args.rules.unwrap_or_else(default_rules_path);
    let rules = match load_rules(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("repair: load rules: {e}");
            return ExitCode::from(4);
        }
    };

    let gate = if args.l0_only {
        classify_one_l0(&args.url, &rules)
    } else {
        #[cfg(feature = "gate_full")]
        {
            let opts = ClassifyOpts {
                tcp_timeout_s: args.tcp_timeout,
                l2_timeout_s: args.l2_timeout,
            };
            classify_one(&args.url, &rules, &opts)
        }
        #[cfg(not(feature = "gate_full"))]
        {
            let _ = (args.tcp_timeout, args.l2_timeout);
            classify_one_l0(&args.url, &rules)
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

    let key = SourceKey::new(args.url.trim());
    let repo = McpSourceRepository::new(Arc::clone(&client));
    let source = match repo.get(&key) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("repair: get_source: {e}");
            return ExitCode::from(2);
        }
    };

    let mut builder = source_spine::RepairContext::builder(key.clone(), source)
        .gate(gate.clone())
        .dry_run(args.dry_run)
        .no_verify(args.no_verify);

    if let Some(html_path) = &args.html {
        match std::fs::read(html_path) {
            Ok(body) => match Url::new(args.url.trim()) {
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
    } else if args.prefetch {
        match Url::new(args.url.trim()) {
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
    let result = match run_repair_oneshot(
        ctx,
        &ports,
        PlanOrPlugin::Plugin(&plugin),
        GateInput::Injected(gate),
        Some(&reg),
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
