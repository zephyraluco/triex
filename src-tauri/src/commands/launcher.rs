use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::settings::open_settings_window;
use crate::error::{AppError, AppResult};
use crate::platform::launch::{launch_search_item, reveal_in_explorer};
use crate::platform::window::{
    capture_foreground_hwnd, position_on_cursor_monitor, restore_foreground_hwnd,
    set_logical_height,
};
use crate::plugins::manager::{close_active_plugin, open_plugin};
use crate::search::index::ItemKind;
use crate::state::AppState;

const LAUNCHER_LOGICAL_WIDTH: f64 = 680.0;

#[tauri::command]
pub fn launch_item(
    item_id: String,
    as_admin: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let raw_item = {
        let engine = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;
        engine
            .get_raw_item(&item_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Item '{item_id}' not found")))?
    };

    // Built-in launcher settings shortcut opens the dedicated Settings window.
    if raw_item.exec_path == "triex:settings" {
        hide_main_launcher(&app, false);
        open_settings_window(&app)?;
        return Ok(());
    }

    // Plugin items mount inside the Multiwebview host instead of spawning an OS process.
    if raw_item.kind == ItemKind::Plugin {
        let mut parts = raw_item.exec_path.splitn(2, ':');
        let plugin_id = parts.next().unwrap_or_default();
        let feature_code = parts.next().unwrap_or_default();
        open_plugin(&app, plugin_id, feature_code)?;
        if let Ok(record) = state.db.record_item_launch(&item_id)
            && let Ok(mut engine) = state.search.lock()
        {
            engine.update_item_usage(&item_id, record);
        }
        return Ok(());
    }

    launch_search_item(&raw_item, as_admin.unwrap_or(false))?;

    if let Ok(record) = state.db.record_item_launch(&item_id)
        && let Ok(mut engine) = state.search.lock()
    {
        engine.update_item_usage(&item_id, record);
    }

    hide_main_launcher(&app, false);
    Ok(())
}

#[tauri::command]
pub fn open_item_folder(item_id: String, state: State<'_, AppState>) -> AppResult<()> {
    let raw_item = {
        let engine = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;
        engine
            .get_raw_item(&item_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Item '{item_id}' not found")))?
    };

    let target_path = if !raw_item.subtitle.is_empty()
        && std::path::Path::new(&raw_item.subtitle).exists()
    {
        &raw_item.subtitle
    } else {
        &raw_item.exec_path
    };

    reveal_in_explorer(target_path)
}

#[tauri::command]
pub fn resize_window(height: f64, app: AppHandle) -> AppResult<()> {
    let clamped = height.clamp(64.0, 620.0);
    if let Some(window) = app.get_webview_window("main") {
        set_logical_height(&window, LAUNCHER_LOGICAL_WIDTH, clamped);
    }
    Ok(())
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> AppResult<()> {
    hide_main_launcher(&app, true);
    Ok(())
}

/// Shows the launcher positioned on the monitor containing the mouse cursor.
pub fn show_main_launcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if !window.is_visible().unwrap_or(false) {
        state.store_prev_hwnd(capture_foreground_hwnd());
    }

    position_on_cursor_monitor(&window);
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    let _ = app.emit("triex://launcher-shown", ());
    Ok(())
}

/// Hides the launcher, closes any active embedded plugin, and optionally restores focus.
pub fn hide_main_launcher(app: &AppHandle, restore_focus: bool) {
    let _ = close_active_plugin(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let state = app.state::<AppState>();
    let prev = state.take_prev_hwnd();
    if restore_focus {
        restore_foreground_hwnd(prev);
    }
}

pub fn toggle_main_launcher(app: &AppHandle) -> AppResult<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        hide_main_launcher(app, true);
    } else {
        show_main_launcher(app)?;
    }
    Ok(())
}
