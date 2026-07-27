//! Video / file bounded context (§14.11 Phase F skeleton).
//!
//! Route table parity with `scripts/video_prefilter.py`; light smells from
//! `scripts/video_repair_one.py`. Does **not** share novel TOC adapters.

mod error;
mod route;
mod smell;

pub use error::VideoError;
pub use route::{
    classify_type3_or_4, is_video_or_file, is_video_or_file_type, load_routes, route_url,
    RouteRule, TypeClass, VideoRoute,
};
pub use smell::smell_video;

pub type Result<T> = std::result::Result<T, VideoError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn routes_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/video_source_routes.json")
    }

    fn load_default() -> Vec<RouteRule> {
        load_routes(routes_path()).expect("load video_source_routes.json")
    }

    #[test]
    fn golden_taopian_from_routes_file() {
        let routes = load_default();
        let r = route_url("https://www.taopianzy.com/index.php", &routes);
        assert_eq!(r.flow, "video");
        assert_eq!(r.reason, "m3u8_movie");
        assert_eq!(r.route_id.as_deref(), Some("taopian"));
        assert!(r.is_video_or_file);
        assert!(is_video_or_file("https://taopianzy.com/", &routes));
    }

    #[test]
    fn golden_uku_and_nangua() {
        let routes = load_default();
        let u = route_url("https://ukuzy.com/vod/play", &routes);
        assert_eq!(u.route_id.as_deref(), Some("uku"));
        assert_eq!(u.reason, "m3u8_movie");
        assert!(u.is_video_or_file);

        let n = route_url("https://nanguady.cc/index", &routes);
        assert_eq!(n.route_id.as_deref(), Some("nangua"));
        assert_eq!(n.reason, "vod_movie");
    }

    #[test]
    fn golden_generic_zy_and_novel_default() {
        let routes = load_default();
        // pattern: zy\.com/home|资源网|影视
        let g = route_url("https://foo.zy.com/home/index", &routes);
        assert_eq!(g.route_id.as_deref(), Some("generic_zy"));
        assert_eq!(g.reason, "name_or_path_media");

        let zh = route_url("某某影视资源网", &routes);
        assert!(zh.is_video_or_file);

        let novel = route_url("https://benign-novel.example/search?q=1", &routes);
        assert_eq!(novel.flow, "novel");
        assert_eq!(novel.reason, "default_novel");
        assert!(!novel.is_video_or_file);
        assert!(!is_video_or_file("https://benign-novel.example/", &routes));
    }

    #[test]
    fn type_3_4_classify() {
        assert!(is_video_or_file_type(Some(3)));
        assert!(is_video_or_file_type(Some(4)));
        assert!(!is_video_or_file_type(Some(0)));
        assert!(!is_video_or_file_type(None));

        let routes = load_default();
        let c = classify_type3_or_4("https://example.com/", Some(4), &routes);
        assert!(c.is_video_or_file);
        assert_eq!(c.reason, "type_video");

        let f = classify_type3_or_4("https://example.com/", Some(3), &routes);
        assert_eq!(f.reason, "type_file");

        let via_route = classify_type3_or_4("https://taopianzy.com/", Some(0), &routes);
        assert!(via_route.is_video_or_file);
        assert_eq!(via_route.reason, "m3u8_movie");
    }

    #[test]
    fn all_routes_file_entries_compile() {
        let routes = load_default();
        assert!(
            routes.len() >= 4,
            "expected at least 4 routes from video_source_routes.json, got {}",
            routes.len()
        );
        for rule in &routes {
            assert!(!rule.pattern.is_empty());
            assert!(!rule.id.is_empty());
        }
    }
}
