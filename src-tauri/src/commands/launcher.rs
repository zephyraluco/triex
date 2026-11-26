use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Webview};

use crate::commands::menu::{popup_brand_context_menu, popup_item_context_menu};
use crate::commands::settings::{open_settings_window, toggle_pin};
use crate::error::{AppError, AppResult};
use crate::platform::launch::{launch_search_item, reveal_in_explorer};
use crate::platform::window::{
    capture_foreground_hwnd, is_window_or_child_foreground, position_on_cursor_monitor,
    restore_foreground_hwnd, set_logical_height, set_shell_webview_bounds,
};
use crate::plugins::{
    manager::{close_active_plugin, open_plugin},
    permission::require_host_webview,
};
use crate::search::index::ItemKind;
use crate::state::AppState;

pub const LAUNCHER_LOGICAL_WIDTH: f64 = 800.0;
pub const HEADER_LOGICAL_HEIGHT: f64 = 64.0;

pub fn launch_item_by_id(app: &AppHandle, item_id: &str, as_admin: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    let raw_item = {
        let engine = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;
        engine
            .get_raw_item(item_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Item '{item_id}' not found")))?
    };

    if raw_item.exec_path == "triex:settings" {
        open_settings_window(app)?;
        return Ok(());
    }

    if raw_item.kind == ItemKind::Plugin {
        let mut parts = raw_item.exec_path.splitn(2, ':');
        let plugin_id = parts.next().unwrap_or_default();
        let feature_code = parts.next().unwrap_or_default();
        open_plugin(app, plugin_id, feature_code)?;
        if let Ok(record) = state.db.record_item_launch(item_id)
            && let Ok(mut engine) = state.search.lock()
        {
            engine.update_item_usage(item_id, record);
        }
        return Ok(());
    }

    launch_search_item(&raw_item, as_admin)?;

    if let Ok(record) = state.db.record_item_launch(item_id)
        && let Ok(mut engine) = state.search.lock()
    {
        engine.update_item_usage(item_id, record);
    }

    hide_main_launcher(app, false);
    Ok(())
}

pub fn open_folder_by_id(app: &AppHandle, item_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let raw_item = {
        let engine = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;
        engine
            .get_raw_item(item_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Item '{item_id}' not found")))?
    };

    reveal_in_explorer(&raw_item.exec_path)
}

pub fn remove_recent_by_id(app: &AppHandle, item_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    state.db.remove_item_usage(item_id)?;
    if let Ok(mut engine) = state.search.lock() {
        engine.update_item_usage(
            item_id,
            crate::storage::apps_repo::UsageRecord {
                launch_count: 0,
                last_used_at: 0,
            },
        );
    }
    let _ = app.emit("triex://index-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn launch_item(
    item_id: String,
    as_admin: Option<bool>,
    webview: Webview,
    app: AppHandle,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    launch_item_by_id(&app, &item_id, as_admin.unwrap_or(false))
}

#[tauri::command]
pub fn open_item_folder(item_id: String, webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    open_folder_by_id(&app, &item_id)
}

#[tauri::command]
pub fn toggle_pin_item(
    item_id: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<Vec<String>> {
    require_host_webview(&webview)?;
    toggle_pin(&app, item_id)
}

#[tauri::command]
pub fn remove_recent_item(item_id: String, webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    remove_recent_by_id(&app, &item_id)
}

#[tauri::command]
pub fn show_item_context_menu(
    item_id: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    popup_item_context_menu(&app, &item_id)
}

#[tauri::command]
pub fn show_brand_context_menu(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    popup_brand_context_menu(&app)
}

#[tauri::command]
pub fn resize_window(height: f64, webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    let state = app.state::<AppState>();
    if state.settings_open() || state.active_plugin().is_some() {
        return Ok(());
    }

    let clamped = height.clamp(HEADER_LOGICAL_HEIGHT, 620.0);
    if let Some(window) = app.get_window("main") {
        set_logical_height(&window, LAUNCHER_LOGICAL_WIDTH, clamped);
        set_shell_webview_bounds(&app, LAUNCHER_LOGICAL_WIDTH, clamped);
    }
    Ok(())
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> AppResult<()> {
    hide_main_launcher(&app, true);
    Ok(())
}

#[tauri::command]
pub fn complete_hide(epoch: u64, webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    complete_offscreen_hide(&app, epoch);
    Ok(())
}

fn complete_offscreen_hide(app: &AppHandle, epoch: u64) {
    let state = app.state::<AppState>();
    if state.visibility_epoch.load(Ordering::SeqCst) == epoch
        && !state.is_shown.load(Ordering::SeqCst)
        && let Some(window) = app.get_window("main")
    {
        let _ = window.hide();
    }
}

/// Shows the launcher positioned on the monitor containing the mouse cursor.
pub fn show_main_launcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(window) = app.get_window("main") else {
        return Ok(());
    };

    let was_shown = state.is_shown.swap(true, Ordering::SeqCst);
    state.visibility_epoch.fetch_add(1, Ordering::SeqCst);

    if !was_shown {
        state.store_prev_hwnd(capture_foreground_hwnd());
    }

    position_on_cursor_monitor(&window);
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    if !state.settings_open()
        && state.active_plugin().is_none()
        && let Some(shell_wv) = app.get_webview("main")
    {
        let _ = shell_wv.set_focus();
    }
    let _ = app.emit("triex://launcher-shown", ());
    Ok(())
}

/// Hides the launcher cleanly without leaving a stale WebView2 SwapChain frame:
/// moves the window offscreen first so Chromium commits a fresh default home frame,
/// then hides it via `complete_offscreen_hide` (with a 160ms fallback guard).
pub fn hide_main_launcher(app: &AppHandle, restore_focus: bool) {
    let state = app.state::<AppState>();
    if !state.is_shown.swap(false, Ordering::SeqCst) {
        return;
    }

    let epoch = state.visibility_epoch.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = close_active_plugin(app);

    if let Some(window) = app.get_window("main") {
        let _ = window.set_position(PhysicalPosition::new(-32_000, -32_000));
    }

    let prev = state.take_prev_hwnd();
    if restore_focus {
        restore_foreground_hwnd(prev);
    }

    let _ = app.emit("triex://launcher-hidden", epoch);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(160)).await;
        complete_offscreen_hide(&handle, epoch);
    });
}

pub fn toggle_main_launcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(window) = app.get_window("main") else {
        return Ok(());
    };

    if state.is_shown.load(Ordering::SeqCst) && is_window_or_child_foreground(&window) {
        hide_main_launcher(app, true);
    } else {
        show_main_launcher(app)?;
    }
    Ok(())
}
