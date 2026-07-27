//! Video route lookup via `source_video` + `config/video_source_routes.json`.

use source_video::{load_routes, route_url};
use std::path::PathBuf;
use std::process::ExitCode;

fn default_routes_path() -> PathBuf {
    let candidates = [
        PathBuf::from("config/video_source_routes.json"),
        PathBuf::from("../config/video_source_routes.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/video_source_routes.json"),
    ];
    for p in candidates {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/video_source_routes.json")
}

pub fn run_video_route(url: &str, routes_path: Option<PathBuf>) -> ExitCode {
    let path = routes_path.unwrap_or_else(default_routes_path);
    let routes = match load_routes(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("video-route: load {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    match serde_json::to_string(&route_url(url, &routes)) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("video-route: serialize: {e}");
            ExitCode::FAILURE
        }
    }
}
