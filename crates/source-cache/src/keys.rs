use sha2::{Digest, Sha256};
use url::Url;

/// First 24 hex chars of sha256(url) — matches Python `url_key`.
pub fn url_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    hex::encode(digest)[..24].to_string()
}

pub fn host_of(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_key_stable_length() {
        let k = url_key("https://example.com/a");
        assert_eq!(k.len(), 24);
        assert_eq!(k, url_key("https://example.com/a"));
    }

    #[test]
    fn host_lower() {
        assert_eq!(host_of("https://ExAmPle.COM/x"), "example.com");
    }
}
