//! L2 HTTP GET + deadish sniff — parity with `repair_prefilter.probe_l2`.

use regex::Regex;
use source_types::{HostKey, L2Probe, Url};
use std::io::Read;
use std::time::{Duration, Instant};

use crate::sniff::sniff_dead_html;
use crate::url_util::{ensure_scheme, host_of};

const UA: &str = "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
const BODY_LIMIT: u64 = 12_000;

/// GET homepage (not HEAD-only) and sniff parking / wall / shell / host migrate.
pub fn probe_l2(url: &str, timeout_s: f64) -> L2Probe {
    let probe = ensure_scheme(url);
    let start = Instant::now();
    let timeout = Duration::from_secs_f64(timeout_s.max(0.1));

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout)
        .user_agent(UA)
        .build();

    let result = agent
        .get(&probe)
        .set("Accept", "text/html,application/xhtml+xml")
        .call();

    match result {
        Ok(resp) => finish_ok(resp, &probe, start),
        Err(ureq::Error::Status(code, resp)) => finish_http_err(code, resp, &probe, start),
        Err(e) => L2Probe {
            ok: false,
            status: None,
            final_url: None,
            title: None,
            bytes: None,
            deadish: None,
            host_migrated: None,
            from_host: None,
            to_host: None,
            snippet: Some(truncate(&e.to_string(), 200)),
        },
    }
}

fn finish_ok(resp: ureq::Response, probe: &str, _start: Instant) -> L2Probe {
    let status = resp.status();
    let final_s = resp.get_url().to_string();
    let text = read_body(resp);
    build_probe(status, probe, &final_s, &text)
}

fn finish_http_err(code: u16, resp: ureq::Response, probe: &str, _start: Instant) -> L2Probe {
    if code >= 500 {
        return L2Probe {
            ok: false,
            status: Some(code),
            final_url: Url::new(probe).ok(),
            title: None,
            bytes: None,
            deadish: None,
            host_migrated: None,
            from_host: None,
            to_host: None,
            snippet: Some(format!("http_{code}")),
        };
    }
    // 4xx: still sniff body (Python path)
    let final_s = resp.get_url().to_string();
    let final_s = if final_s.is_empty() {
        probe.to_string()
    } else {
        final_s
    };
    let text = read_body(resp);
    let title = extract_title(&text);
    let reason = sniff_dead_html(&text, &final_s, &title);
    let ok = reason.is_none() && code < 400;
    L2Probe {
        ok,
        status: Some(code),
        final_url: Url::new(&final_s).ok(),
        title: Some(truncate(&title, 80)),
        bytes: Some(text.len() as u64),
        deadish: reason,
        host_migrated: None,
        from_host: None,
        to_host: None,
        snippet: None,
    }
}

fn build_probe(status: u16, probe: &str, final_s: &str, text: &str) -> L2Probe {
    let title = extract_title(text);
    let reason = sniff_dead_html(text, final_s, &title);
    let src_host = host_of(probe);
    let fin_host = host_of(final_s);
    let migrated = match (&src_host, &fin_host) {
        (Some(a), Some(b)) => {
            a.to_ascii_lowercase().trim_end_matches('.')
                != b.to_ascii_lowercase().trim_end_matches('.')
        }
        _ => false,
    };
    let ok = (200..400).contains(&status) && reason.is_none();
    let mut out = L2Probe {
        ok,
        status: Some(status),
        final_url: Url::new(final_s).ok(),
        title: Some(truncate(&title, 80)),
        bytes: Some(text.len() as u64),
        deadish: reason,
        host_migrated: None,
        from_host: None,
        to_host: None,
        snippet: None,
    };
    if migrated {
        out.host_migrated = Some(true);
        out.from_host = src_host.map(HostKey::new);
        out.to_host = fin_host.map(HostKey::new);
    }
    out
}

fn read_body(resp: ureq::Response) -> String {
    let mut buf = Vec::new();
    let mut reader = resp.into_reader().take(BODY_LIMIT);
    let _ = reader.read_to_end(&mut buf);
    // ureq `gzip` feature decompresses Content-Encoding; no manual inflate.
    String::from_utf8_lossy(&buf).into_owned()
}

fn extract_title(text: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)<title[^>]*>([^<]+)").expect("title regex")
    });
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Test helper: build L2-shaped sniff decision without network.
#[cfg(test)]
pub fn probe_from_html_fixture(status: u16, final_url: &str, html: &str) -> L2Probe {
    build_probe(status, final_url, final_url, html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sniff::sniff_dead_html;

    #[test]
    fn fixture_wall_sets_deadish() {
        let html = "<html><body>password protected area</body></html>";
        let p = probe_from_html_fixture(200, "https://wall.example/", html);
        assert!(!p.ok);
        assert!(p.deadish.as_ref().unwrap().starts_with("wall:"));
    }

    #[test]
    fn fixture_park_sets_deadish() {
        let html = "<html>domain expired — renew at registrar</html>";
        let p = probe_from_html_fixture(200, "https://old.example/", html);
        assert!(!p.ok);
        assert!(p.deadish.as_ref().unwrap().starts_with("deadish:"));
    }

    #[test]
    fn host_migrate_detected() {
        let html = "<html><title>ok</title><body>novel home</body></html>";
        let p = build_probe(
            200,
            "https://old.org/",
            "https://www.new.com/",
            html,
        );
        assert!(p.ok);
        assert_eq!(p.host_migrated, Some(true));
        assert_eq!(p.from_host.as_ref().unwrap().as_str(), "old.org");
        assert_eq!(p.to_host.as_ref().unwrap().as_str(), "www.new.com");
        assert!(sniff_dead_html(html, "https://www.new.com/", "ok").is_none());
    }

    #[test]
    #[ignore = "live network"]
    fn live_example_com_http() {
        let p = probe_l2("https://example.com/", 4.0);
        assert!(p.ok, "{p:?}");
        assert_eq!(p.status, Some(200));
    }
}
