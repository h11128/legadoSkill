//! Shared URL helpers for L1/L2 (parity with `repair_prefilter` clean_url / host_of).

use source_types::Url;

/// Strip fragment and trim — Python `clean_url`.
pub fn clean_url(url: &str) -> String {
    url.split('#').next().unwrap_or(url).trim().to_string()
}

/// Ensure `http(s)://` prefix for parsing.
pub fn ensure_scheme(url: &str) -> String {
    let c = clean_url(url);
    if c.contains("://") {
        c
    } else {
        format!("http://{c}")
    }
}

/// Hostname only (no port) — Python `host_of` / `urlparse.hostname`.
pub fn host_of(url: &str) -> Option<String> {
    let raw = ensure_scheme(url);
    let parsed = ::url::Url::parse(&raw).ok()?;
    parsed.host_str().map(|h| h.to_string())
}

/// Typed URL for GateResult; fall back like L0 `to_url`.
pub fn to_gate_url(raw: &str) -> Url {
    match Url::new(raw.trim()) {
        Ok(u) => u,
        Err(_) => {
            let padded = ensure_scheme(raw);
            Url::new(padded.trim())
                .unwrap_or_else(|_| Url::new("http://invalid.invalid/").expect("fallback url"))
        }
    }
}

/// Scheme default port, or explicit URL port when present.
///
/// Python always uses 443/80 from scheme and ignores `:port` — we prefer the
/// URL port when set (intentional delta; better for non-standard hosts).
pub fn connect_port(url: &str) -> u16 {
    let raw = ensure_scheme(url);
    if let Ok(parsed) = ::url::Url::parse(&raw) {
        if let Some(p) = parsed.port() {
            return p;
        }
        return match parsed.scheme() {
            "https" => 443,
            _ => 80,
        };
    }
    if raw.to_ascii_lowercase().starts_with("https") {
        443
    } else {
        80
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_strips_hash() {
        assert_eq!(clean_url(" https://a.com/x#y "), "https://a.com/x");
    }

    #[test]
    fn host_of_hostname_only() {
        // rust-url lowercases host (WHATWG); Python urlparse may preserve case,
        // but classify compares with .lower() anyway.
        assert_eq!(
            host_of("https://WWW.Example.com:8443/p").as_deref(),
            Some("www.example.com")
        );
        assert_eq!(
            host_of("bare.example/path").as_deref(),
            Some("bare.example")
        );
    }

    #[test]
    fn connect_port_respects_explicit() {
        assert_eq!(connect_port("https://a.com:8443/"), 8443);
        assert_eq!(connect_port("https://a.com/"), 443);
        assert_eq!(connect_port("http://a.com/"), 80);
    }
}
