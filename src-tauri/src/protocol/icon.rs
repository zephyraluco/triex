use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

use crate::state::AppState;

/// Handles `icon://localhost/<icon_key>` requests by streaming cached PNG icons from disk.
pub fn handle_icon_protocol(
    app: &tauri::AppHandle,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let raw_key = request
        .uri()
        .path()
        .trim_start_matches('/')
        .trim_end_matches(".png");

    // Only allow strict hexadecimal hash keys to prevent path traversal.
    if raw_key.is_empty()
        || raw_key.len() > 64
        || !raw_key.chars().all(|c| c.is_ascii_hexdigit())
    {
        return not_found();
    }

    let state = app.state::<AppState>();
    let icon_path = state.paths.icon_file(raw_key);

    match std::fs::read(&icon_path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "public, max-age=86400")
            .body(bytes)
            .unwrap_or_else(|_| not_found()),
        Err(_) => not_found(),
    }
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}
