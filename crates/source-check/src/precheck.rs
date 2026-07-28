//! DNS/HTTP bulk precheck for dead hosts.

use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PrecheckRow {
    pub url: String,
    pub ok: bool,
    pub status: Option<u16>,
    pub error: Option<String>,
}

pub fn precheck_urls(urls: &[String], timeout_s: f64) -> Vec<PrecheckRow> {
    let timeout = Duration::from_secs_f64(timeout_s.max(0.5));
    urls.iter()
        .map(|url| {
            let agent = ureq::AgentBuilder::new().timeout(timeout).build();
            match agent.get(url).call() {
                Ok(resp) => PrecheckRow {
                    url: url.clone(),
                    ok: resp.status() >= 200 && resp.status() < 500,
                    status: Some(resp.status()),
                    error: None,
                },
                Err(e) => PrecheckRow {
                    url: url.clone(),
                    ok: false,
                    status: None,
                    error: Some(format!("{e}")),
                },
            }
        })
        .collect()
}

pub fn precheck_json(urls: &[String], timeout_s: f64) -> Value {
    json!({
        "rows": precheck_urls(urls, timeout_s).into_iter().map(|r| json!({
            "url": r.url,
            "ok": r.ok,
            "status": r.status,
            "error": r.error,
        })).collect::<Vec<_>>()
    })
}
