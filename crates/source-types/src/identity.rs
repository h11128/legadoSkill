//! Identity newtypes and opaque BookSource payloads (§3.1).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::TypeError;

/// Absolute `http(s)://…` URL. Trimmed on every construct.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Url(String);

impl Url {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, TypeError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(TypeError::InvalidUrl("empty".into()));
        }
        let lower = trimmed.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(TypeError::InvalidUrl(format!(
                "must be http(s): {trimmed}"
            )));
        }
        // Normalize scheme casing for downstream parsers (Url crate).
        let normalized = if trimmed.len() >= 8 && lower.starts_with("https://") {
            format!("https://{}", &trimmed[8..])
        } else if trimmed.len() >= 7 && lower.starts_with("http://") {
            format!("http://{}", &trimmed[7..])
        } else {
            trimmed.to_string()
        };
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn host_key(&self) -> Result<HostKey, TypeError> {
        HostKey::from_url(self)
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// `urlparse(url).netloc.lower()` — port kept unless App strips it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostKey(String);

impl HostKey {
    pub fn new(raw: impl AsRef<str>) -> Self {
        Self(raw.as_ref().trim().to_ascii_lowercase())
    }

    pub fn from_url(url: &Url) -> Result<Self, TypeError> {
        let parsed = ::url::Url::parse(url.as_str())
            .map_err(|e| TypeError::InvalidUrl(e.to_string()))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| TypeError::InvalidUrl("missing host".into()))?;
        let key = match parsed.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        };
        Ok(Self(key.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HostKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Primary key = trimmed `bookSourceUrl`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceKey(String);

impl SourceKey {
    pub fn new(raw: impl AsRef<str>) -> Self {
        Self(raw.as_ref().trim().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_url(&self) -> Result<Url, TypeError> {
        Url::new(&self.0)
    }
}

impl std::fmt::Display for SourceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque App `BookSource.kt` JSON — no second field vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BookSource(Value);

impl BookSource {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }

    pub fn source_key(&self) -> Option<SourceKey> {
        self.0
            .get("bookSourceUrl")
            .and_then(|v| v.as_str())
            .map(SourceKey::new)
    }

    /// Typed path getters (§14.8) — read-only views into App JSON.
    pub fn search_url(&self) -> Option<&str> {
        self.0.get("searchUrl").and_then(|v| v.as_str())
    }

    pub fn rule_search_book_list(&self) -> Option<&str> {
        self.0
            .pointer("/ruleSearch/bookList")
            .and_then(|v| v.as_str())
    }

    pub fn rule_toc_chapter_list(&self) -> Option<&str> {
        self.0
            .pointer("/ruleToc/chapterList")
            .and_then(|v| v.as_str())
    }

    pub fn rule_content_content(&self) -> Option<&str> {
        self.0
            .pointer("/ruleContent/content")
            .and_then(|v| v.as_str())
    }
}

/// Subset of BookSource fields an adapter may set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartialBookSource(Value);

impl PartialBookSource {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

/// Dotted App paths: `searchUrl`, `ruleSearch.bookList` (§3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonPath(String);

impl JsonPath {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for JsonPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for JsonPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_trims_and_rejects_non_http() {
        let u = Url::new("  https://Example.com/a  ").unwrap();
        assert_eq!(u.as_str(), "https://Example.com/a");
        assert!(Url::new("ftp://x").is_err());
    }

    #[test]
    fn host_key_lowercases_netloc() {
        let u = Url::new("https://Example.COM:8443/path").unwrap();
        assert_eq!(u.host_key().unwrap().as_str(), "example.com:8443");
    }
}
