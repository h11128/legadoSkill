//! Route media hosts to video/file repair flow — parity with `scripts/video_prefilter.py`.

use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::{Result, VideoError};

/// One rule from `config/video_source_routes.json`.
#[derive(Debug, Clone)]
pub struct RouteRule {
    pub id: String,
    pub pattern: String,
    pub flow: String,
    pub reason: String,
    re: Regex,
}

#[derive(Debug, Deserialize)]
struct RoutesFile {
    #[serde(default)]
    routes: Vec<RawRoute>,
}

#[derive(Debug, Deserialize)]
struct RawRoute {
    id: Option<String>,
    pattern: Option<String>,
    flow: Option<String>,
    reason: Option<String>,
}

/// Result of matching a URL against the video route table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VideoRoute {
    pub url: String,
    /// `"video"` or `"novel"` (default).
    pub flow: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    /// True when flow is video (type 3/4 media path, not novel TOC).
    pub is_video_or_file: bool,
}

/// Load `config/video_source_routes.json` (or any same-shaped file).
pub fn load_routes(path: impl AsRef<Path>) -> Result<Vec<RouteRule>> {
    let text = fs::read_to_string(path.as_ref())?;
    let file: RoutesFile = serde_json::from_str(&text)?;
    let mut out = Vec::with_capacity(file.routes.len());
    for raw in file.routes {
        let pat = raw.pattern.unwrap_or_default();
        if pat.is_empty() {
            continue;
        }
        let id = raw.id.clone().unwrap_or_default();
        let flow = raw.flow.clone().unwrap_or_else(|| "video".into());
        let reason = raw
            .reason
            .clone()
            .or(raw.id.clone())
            .unwrap_or_else(|| "video_route".into());
        let re = RegexBuilder::new(&pat)
            .case_insensitive(true)
            .build()
            .map_err(|e| VideoError::Msg(format!("bad pattern {id:?}: {e}")))?;
        out.push(RouteRule {
            id,
            pattern: pat,
            flow,
            reason,
            re,
        });
    }
    Ok(out)
}

/// First matching route, or novel default (same order as Python `route_url`).
pub fn route_url(url: &str, routes: &[RouteRule]) -> VideoRoute {
    for rule in routes {
        if rule.re.is_match(url) {
            let is_media =
                rule.flow.eq_ignore_ascii_case("video") || rule.flow.eq_ignore_ascii_case("file");
            return VideoRoute {
                url: url.to_string(),
                flow: rule.flow.clone(),
                reason: rule.reason.clone(),
                route_id: Some(rule.id.clone()),
                is_video_or_file: is_media,
            };
        }
    }
    VideoRoute {
        url: url.to_string(),
        flow: "novel".into(),
        reason: "default_novel".into(),
        route_id: None,
        is_video_or_file: false,
    }
}

/// True when URL matches a video/file route (not novel default).
pub fn is_video_or_file(url: &str, routes: &[RouteRule]) -> bool {
    route_url(url, routes).is_video_or_file
}

/// `bookSourceType` 3 = file, 4 = video (Legado); treat both as media flow.
pub fn is_video_or_file_type(book_source_type: Option<i64>) -> bool {
    matches!(book_source_type, Some(3) | Some(4))
}

/// Classify a source by type field and/or URL route.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TypeClass {
    pub is_video_or_file: bool,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_source_type: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<VideoRoute>,
}

/// Prefer explicit type 3/4; else fall back to URL route table.
pub fn classify_type3_or_4(
    url: &str,
    book_source_type: Option<i64>,
    routes: &[RouteRule],
) -> TypeClass {
    if is_video_or_file_type(book_source_type) {
        let kind = if book_source_type == Some(3) {
            "type_file"
        } else {
            "type_video"
        };
        return TypeClass {
            is_video_or_file: true,
            reason: kind.into(),
            book_source_type,
            route: Some(route_url(url, routes)),
        };
    }
    let route = route_url(url, routes);
    TypeClass {
        is_video_or_file: route.is_video_or_file,
        reason: route.reason.clone(),
        book_source_type,
        route: Some(route),
    }
}
