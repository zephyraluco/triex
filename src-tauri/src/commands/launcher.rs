use std::collections::HashMap;
use std::sync::atomic::Ordering;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::settings::open_settings_window;
use crate::error::{AppError, AppResult};
use crate::platform::clipboard::write_clipboard_text;
use crate::platform::launch::{launch_search_item, reveal_in_explorer};
use crate::platform::window::{
    capture_foreground_hwnd, position_on_cursor_monitor, restore_foreground_hwnd,
    set_logical_height,
};
use crate::plugins::manager::{close_active_plugin, open_plugin};
use crate::search::index::ItemKind;
use crate::state::{AppState, ContextMenuAction};

const LAUNCHER_LOGICAL_WIDTH: f64 = 800.0;

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

    // Built-in launcher settings shortcut opens the embedded Settings view inside the main window.
    if raw_item.exec_path == "triex:settings" {
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
pub fn toggle_pin_item(item_id: String, app: AppHandle, state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let mut settings = state.current_settings();
    if let Some(pos) = settings.pinned_items.iter().position(|id| id == &item_id) {
        settings.pinned_items.remove(pos);
    } else {
        settings.pinned_items.push(item_id);
    }

    state.db.save_settings(&settings)?;
    let pinned = settings.pinned_items.clone();
    if let Ok(mut guard) = state.settings.write() {
        *guard = settings.clone();
    }
    let _ = app.emit("triex://settings-updated", &settings);
    Ok(pinned)
}

#[tauri::command]
pub fn remove_recent_item(item_id: String, app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.db.remove_item_usage(&item_id)?;
    if let Ok(mut engine) = state.search.lock() {
        engine.update_item_usage(
            &item_id,
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
pub fn show_item_context_menu(
    item_id: String,
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

    let settings = state.current_settings();
    let is_pinned = settings.pinned_items.iter().any(|id| id == &item_id);
    let usage = state.db.load_all_usage().unwrap_or_default();
    let has_recent = usage.get(&item_id).is_some_and(|u| u.launch_count > 0);
    let is_win32_app = raw_item.kind == ItemKind::App;
    let copy_text = if !raw_item.subtitle.is_empty() {
        raw_item.subtitle.clone()
    } else {
        raw_item.exec_path.clone()
    };

    let mut actions = HashMap::new();
    let menu = Menu::new(&app)?;

    let open_mi = MenuItem::with_id(&app, "ctx:open", "打开", true, None::<&str>)?;
    actions.insert(
        "ctx:open".to_string(),
        ContextMenuAction::Launch {
            item_id: item_id.clone(),
            as_admin: false,
        },
    );
    menu.append(&open_mi)?;

    let pin_label = if is_pinned {
        "取消固定"
    } else {
        "固定到常用"
    };
    let pin_mi = MenuItem::with_id(&app, "ctx:pin", pin_label, true, None::<&str>)?;
    actions.insert(
        "ctx:pin".to_string(),
        ContextMenuAction::TogglePin {
            item_id: item_id.clone(),
        },
    );
    menu.append(&pin_mi)?;

    if has_recent {
        let unrecent_mi =
            MenuItem::with_id(&app, "ctx:unrecent", "从最近使用移除", true, None::<&str>)?;
        actions.insert(
            "ctx:unrecent".to_string(),
            ContextMenuAction::RemoveRecent {
                item_id: item_id.clone(),
            },
        );
        menu.append(&unrecent_mi)?;
    }

    if is_win32_app {
        let admin_mi =
            MenuItem::with_id(&app, "ctx:admin", "以管理员身份运行", true, None::<&str>)?;
        actions.insert(
            "ctx:admin".to_string(),
            ContextMenuAction::Launch {
                item_id: item_id.clone(),
                as_admin: true,
            },
        );
        menu.append(&admin_mi)?;

        let folder_mi =
            MenuItem::with_id(&app, "ctx:folder", "打开文件所在位置", true, None::<&str>)?;
        actions.insert(
            "ctx:folder".to_string(),
            ContextMenuAction::OpenFolder {
                item_id: item_id.clone(),
            },
        );
        menu.append(&folder_mi)?;
    }

    let copy_mi = MenuItem::with_id(&app, "ctx:copy", "复制路径 / 标识", true, None::<&str>)?;
    actions.insert(
        "ctx:copy".to_string(),
        ContextMenuAction::CopyPath { text: copy_text },
    );
    menu.append(&copy_mi)?;

    let sep = PredefinedMenuItem::separator(&app)?;
    menu.append(&sep)?;

    let rebuild_mi = MenuItem::with_id(&app, "ctx:rebuild", "重建应用索引", true, None::<&str>)?;
    actions.insert("ctx:rebuild".to_string(), ContextMenuAction::RebuildIndex);
    menu.append(&rebuild_mi)?;

    let settings_mi = MenuItem::with_id(&app, "ctx:settings", "偏好设置", true, None::<&str>)?;
    actions.insert("ctx:settings".to_string(), ContextMenuAction::OpenSettings);
    menu.append(&settings_mi)?;

    if let Ok(mut guard) = state.context_menu_actions.lock() {
        *guard = actions;
    }

    if let Some(main_win) = app.get_webview_window("main") {
        state.context_menu_open.store(true, Ordering::SeqCst);
        let popup_res = main_win.popup_menu(&menu);
        state.context_menu_open.store(false, Ordering::SeqCst);
        popup_res?;
    }

    Ok(())
}

#[tauri::command]
pub fn set_settings_view_open(open: bool, state: State<'_, AppState>) {
    state.in_settings_view.store(open, Ordering::SeqCst);
}

#[tauri::command]
pub fn show_brand_context_menu(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let in_settings = state.in_settings_view.load(Ordering::SeqCst);
    let mut actions = HashMap::new();
    let menu = Menu::new(&app)?;

    let settings_label = if in_settings {
        "返回搜索"
    } else {
        "打开设置"
    };
    let settings_mi = MenuItem::with_id(&app, "ctx:brand-settings", settings_label, true, None::<&str>)?;
    actions.insert("ctx:brand-settings".to_string(), ContextMenuAction::OpenSettings);
    menu.append(&settings_mi)?;

    let dash_mi =
        MenuItem::with_id(&app, "ctx:brand-dashboard", "展开 / 收起所有应用", true, None::<&str>)?;
    actions.insert("ctx:brand-dashboard".to_string(), ContextMenuAction::ToggleDashboard);
    menu.append(&dash_mi)?;

    let rebuild_mi = MenuItem::with_id(&app, "ctx:brand-rebuild", "重建应用索引", true, None::<&str>)?;
    actions.insert("ctx:brand-rebuild".to_string(), ContextMenuAction::RebuildIndex);
    menu.append(&rebuild_mi)?;

    let sep = PredefinedMenuItem::separator(&app)?;
    menu.append(&sep)?;

    let quit_mi = MenuItem::with_id(&app, "ctx:brand-quit", "退出 Triex", true, None::<&str>)?;
    actions.insert("ctx:brand-quit".to_string(), ContextMenuAction::QuitApp);
    menu.append(&quit_mi)?;

    if let Ok(mut guard) = state.context_menu_actions.lock() {
        *guard = actions;
    }

    if let Some(main_win) = app.get_webview_window("main") {
        state.context_menu_open.store(true, Ordering::SeqCst);
        let popup_res = main_win.popup_menu(&menu);
        state.context_menu_open.store(false, Ordering::SeqCst);
        popup_res?;
    }

    Ok(())
}

/// Executes an action chosen from the native popup context menu.
pub fn handle_context_menu_event(app: &AppHandle, menu_id: &str) {
    let state = app.state::<AppState>();
    let action = state
        .context_menu_actions
        .lock()
        .ok()
        .and_then(|map| map.get(menu_id).cloned());

    let Some(action) = action else {
        return;
    };

    match action {
        ContextMenuAction::Launch { item_id, as_admin } => {
            if let Err(err) = launch_item(item_id, Some(as_admin), app.clone(), state) {
                let _ = app.emit("triex://status-message", format!("启动失败: {err}"));
            }
        }
        ContextMenuAction::TogglePin { item_id } => {
            let was_pinned = state
                .current_settings()
                .pinned_items
                .iter()
                .any(|id| id == &item_id);
            match toggle_pin_item(item_id, app.clone(), state) {
                Ok(_) => {
                    let msg = if was_pinned {
                        "已取消固定"
                    } else {
                        "已固定到常用"
                    };
                    let _ = app.emit("triex://status-message", msg);
                }
                Err(err) => {
                    let _ = app.emit("triex://status-message", format!("操作失败: {err}"));
                }
            }
        }
        ContextMenuAction::RemoveRecent { item_id } => {
            if remove_recent_item(item_id, app.clone(), state).is_ok() {
                let _ = app.emit("triex://status-message", "已从最近使用移除");
            }
        }
        ContextMenuAction::OpenFolder { item_id } => {
            if let Err(err) = open_item_folder(item_id, state) {
                let _ = app.emit("triex://status-message", format!("打开位置失败: {err}"));
            }
        }
        ContextMenuAction::CopyPath { text } => match write_clipboard_text(&text) {
            Ok(()) => {
                let _ = app.emit("triex://status-message", "已复制路径");
            }
            Err(err) => {
                let _ = app.emit("triex://status-message", format!("复制失败: {err}"));
            }
        },
        ContextMenuAction::ToggleDashboard => {
            let _ = app.emit("triex://toggle-dashboard", ());
        }
        ContextMenuAction::RebuildIndex => {
            let handle = app.clone();
            let _ = app.emit("triex://status-message", "正在重建索引...");
            tauri::async_runtime::spawn(async move {
                if let Ok(status) = crate::commands::search::trigger_rebuild_index(&handle).await {
                    let _ = handle.emit(
                        "triex://status-message",
                        format!("已更新 {} 项", status.total_indexed),
                    );
                }
            });
        }
        ContextMenuAction::OpenSettings => {
            let _ = app.emit("triex://toggle-settings", ());
        }
        ContextMenuAction::QuitApp => {
            app.exit(0);
        }
    }
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

/// Finalizes OS hiding after the frontend has rendered the clean default home frame offscreen.
#[tauri::command]
pub fn complete_hide(epoch: u64, app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    if state.visibility_epoch.load(Ordering::SeqCst) == epoch
        && !state.is_shown.load(Ordering::SeqCst)
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.hide();
    }
    Ok(())
}

/// Shows the launcher positioned on the monitor containing the mouse cursor.
pub fn show_main_launcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
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
    let _ = app.emit("triex://launcher-shown", ());
    Ok(())
}

/// Hides the launcher cleanly without leaving a stale WebView2 SwapChain frame:
/// moves the window offscreen first so Chromium commits a fresh default home frame,
/// then hides it via `complete_hide` (with a 150ms fallback guard).
pub fn hide_main_launcher(app: &AppHandle, restore_focus: bool) {
    let state = app.state::<AppState>();
    if !state.is_shown.swap(false, Ordering::SeqCst) {
        return;
    }

    let epoch = state.visibility_epoch.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = close_active_plugin(app);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_position(tauri::PhysicalPosition::new(-32_000, -32_000));
    }

    let prev = state.take_prev_hwnd();
    if restore_focus {
        restore_foreground_hwnd(prev);
    }

    let _ = app.emit("triex://launcher-hidden", epoch);

    // Fallback in case the frontend callback is delayed.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(160)).await;
        let st = handle.state::<AppState>();
        if st.visibility_epoch.load(Ordering::SeqCst) == epoch
            && !st.is_shown.load(Ordering::SeqCst)
            && let Some(window) = handle.get_webview_window("main")
        {
            let _ = window.hide();
        }
    });
}

pub fn toggle_main_launcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if state.is_shown.load(Ordering::SeqCst) && window.is_focused().unwrap_or(false) {
        hide_main_launcher(app, true);
    } else {
        show_main_launcher(app)?;
    }
    Ok(())
}
