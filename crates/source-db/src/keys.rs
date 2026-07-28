//! URL key helpers — parity with `repair_db.norm_source_key` / `host_key`.

use url::Url;

pub fn norm_source_key(url: &str) -> String {
    url.trim().to_string()
}

/// Host without leading `www.` (matches Python `repair_db.host_key`).
pub fn host_key(url: &str) -> String {
    let mut raw = norm_source_key(url);
    if let Some(i) = raw.find("##") {
        raw.truncate(i);
    }
    if let Some(i) = raw.find('#') {
        raw.truncate(i);
    }
    let raw = raw.trim();
    let with_scheme = if raw.is_empty() {
        return String::new();
    } else if !raw.contains("://") {
        format!("http://{}", raw.trim_start_matches('/'))
    } else {
        raw.to_string()
    };
    let host = Url::parse(&with_scheme)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

pub fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn parse_iso_ts(ts: &str) -> Option<f64> {
    let normalized = ts.replace('Z', "+00:00");
    chrono::DateTime::parse_from_rfc3339(&normalized)
        .or_else(|_| {
            // Accept offsets without colon-style already handled; try from_str via Naive
            chrono::DateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S%.f%z")
        })
        .ok()
        .map(|dt| dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_strips_www_and_fragment() {
        assert_eq!(host_key("https://www.Example.com/a#x"), "example.com");
        assert_eq!(host_key("example.org##js"), "example.org");
        assert_eq!(host_key("bare.site/path"), "bare.site");
    }
}
