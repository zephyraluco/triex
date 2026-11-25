use std::path::Path;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

use crate::config::AppPaths;
use crate::state::AppState;

/// Handles `plugin://<plugin-id>/<asset-path>` (macOS/Linux) and
/// `http://plugin.localhost/<plugin-id>/<asset-path>` (Windows WebView2) requests.
pub fn handle_plugin_protocol(
    app: &tauri::AppHandle,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let uri = request.uri();
    let host = uri.host().unwrap_or_default();
    let raw_path = uri.path().trim_start_matches('/');

    let (plugin_id, rel_path) = if host.eq_ignore_ascii_case("plugin.localhost")
        || host.eq_ignore_ascii_case("localhost")
        || host.is_empty()
    {
        let mut parts = raw_path.splitn(2, '/');
        let id = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default();
        (id, rest)
    } else {
        (host, raw_path)
    };

    if plugin_id.is_empty()
        || !plugin_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return error_response(StatusCode::BAD_REQUEST);
    }

    let rel_path = if rel_path.is_empty() {
        "index.html"
    } else {
        rel_path
    };

    let state = app.state::<AppState>();
    let plugin_root = state.paths.plugin_dir(plugin_id);
    let candidate = plugin_root.join(rel_path);

    if !AppPaths::ensure_within_dir(&plugin_root, &candidate) {
        return error_response(StatusCode::FORBIDDEN);
    }

    match std::fs::read(&candidate) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, guess_mime(&candidate))
            .body(bytes)
            .unwrap_or_else(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(_) => error_response(StatusCode::NOT_FOUND),
    }
}

fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn error_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}
