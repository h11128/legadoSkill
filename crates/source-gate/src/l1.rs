//! L1 TCP/DNS probe — parity with `scripts/repair_prefilter.py` `probe_l1`.

use source_types::L1Probe;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::url_util::{connect_port, host_of};

/// DNS resolve + TCP connect (short timeout). Tries up to 2 distinct IPs.
pub fn probe_l1(url: &str, tcp_timeout_s: f64) -> L1Probe {
    let start = Instant::now();
    let Some(host) = host_of(url) else {
        return fail("bad_host", start);
    };
    if host.is_empty() {
        return fail("bad_host", start);
    }
    if host.len() > 253 || host.split('.').any(|p| p.len() > 63) {
        return fail("invalid_hostname", start);
    }

    let timeout = Duration::from_secs_f64(tcp_timeout_s.max(0.05));
    let addrs = match resolve_ips(&host) {
        Ok(a) if !a.is_empty() => a,
        Ok(_) => return fail("tcp:no_addr", start),
        Err(e) => return fail(&format!("dns:{e}"), start),
    };

    let port = connect_port(url);
    let mut last_err = String::from("tcp:no_addr");
    for ip in addrs.iter().take(2) {
        let addr = SocketAddr::new(*ip, port);
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => {
                return L1Probe {
                    ok: true,
                    error: None,
                    ip: Some(ip.to_string()),
                    latency_ms: Some(elapsed_ms(start)),
                };
            }
            Err(e) => last_err = format!("tcp:{e}"),
        }
    }
    fail(&last_err, start)
}

fn resolve_ips(host: &str) -> std::io::Result<Vec<std::net::IpAddr>> {
    // Port 0 is ignored by getaddrinfo; we only need addresses.
    let iter = (host, 0u16).to_socket_addrs()?;
    let mut out = Vec::new();
    for addr in iter {
        let ip = addr.ip();
        if !out.contains(&ip) {
            out.push(ip);
        }
        if out.len() >= 2 {
            break;
        }
    }
    Ok(out)
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn fail(error: &str, start: Instant) -> L1Probe {
    L1Probe {
        ok: false,
        error: Some(error.to_string()),
        ip: None,
        latency_ms: Some(elapsed_ms(start)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_host_emptyish() {
        let r = probe_l1("http:///", 0.2);
        assert!(!r.ok);
        assert_eq!(r.error.as_deref(), Some("bad_host"));
    }

    #[test]
    fn invalid_hostname_label() {
        let long = "a".repeat(64);
        let url = format!("http://{long}.example/");
        let r = probe_l1(&url, 0.2);
        assert!(!r.ok);
        assert_eq!(r.error.as_deref(), Some("invalid_hostname"));
    }

    #[test]
    #[ignore = "live network"]
    fn live_example_com_ok() {
        let r = probe_l1("https://example.com/", 1.5);
        assert!(r.ok, "{r:?}");
        assert!(r.ip.is_some());
    }
}
