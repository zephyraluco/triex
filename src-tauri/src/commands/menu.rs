use std::collections::HashMap;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::launcher::{launch_item_by_id, open_folder_by_id, remove_recent_by_id};
use crate::commands::search::trigger_rebuild_index;
use crate::commands::settings::{close_settings_view, open_settings_window, toggle_pin};
use crate::error::{AppError, AppResult};
use crate::search::index::ItemKind;
use crate::state::{AppState, ContextMenuAction};

fn popup_on_main_window(
    app: &AppHandle,
    state: &AppState,
    menu: &Menu<tauri::Wry>,
    actions: HashMap<String, ContextMenuAction>,
) -> AppResult<()> {
    if let Ok(mut guard) = state.context_menu_actions.lock() {
        *guard = actions;
    }

    if let Some(main_win) = app.get_window("main") {
        let _blur_guard = state.suppress_blur_guard();
        main_win.popup_menu(menu)?;
    }

    Ok(())
}

pub fn popup_item_context_menu(app: &AppHandle, item_id: &str) -> AppResult<()> {
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

    let settings = state.current_settings();
    let is_pinned = settings.pinned_items.iter().any(|id| id == item_id);
    let usage = state.db.load_all_usage().unwrap_or_default();
    let has_recent = usage.get(item_id).is_some_and(|u| u.launch_count > 0);
    let is_win32_app = raw_item.kind == ItemKind::App;

    let mut actions = HashMap::new();
    let menu = Menu::new(app)?;

    let open_mi = MenuItem::with_id(app, "ctx:open", "打开", true, None::<&str>)?;
    actions.insert(
        "ctx:open".to_string(),
        ContextMenuAction::Launch {
            item_id: item_id.to_string(),
            as_admin: false,
        },
    );
    menu.append(&open_mi)?;

    let pin_label = if is_pinned {
        "取消固定"
    } else {
        "固定到常用"
    };
    let pin_mi = MenuItem::with_id(app, "ctx:pin", pin_label, true, None::<&str>)?;
    actions.insert(
        "ctx:pin".to_string(),
        ContextMenuAction::TogglePin {
            item_id: item_id.to_string(),
        },
    );
    menu.append(&pin_mi)?;

    if has_recent {
        let unrecent_mi =
            MenuItem::with_id(app, "ctx:unrecent", "从最近使用移除", true, None::<&str>)?;
        actions.insert(
            "ctx:unrecent".to_string(),
            ContextMenuAction::RemoveRecent {
                item_id: item_id.to_string(),
            },
        );
        menu.append(&unrecent_mi)?;
    }

    if is_win32_app {
        let admin_mi =
            MenuItem::with_id(app, "ctx:admin", "以管理员身份运行", true, None::<&str>)?;
        actions.insert(
            "ctx:admin".to_string(),
            ContextMenuAction::Launch {
                item_id: item_id.to_string(),
                as_admin: true,
            },
        );
        menu.append(&admin_mi)?;

        let folder_mi =
            MenuItem::with_id(app, "ctx:folder", "打开文件所在位置", true, None::<&str>)?;
        actions.insert(
            "ctx:folder".to_string(),
            ContextMenuAction::OpenFolder {
                item_id: item_id.to_string(),
            },
        );
        menu.append(&folder_mi)?;
    }

    let sep = PredefinedMenuItem::separator(app)?;
    menu.append(&sep)?;

    let rebuild_mi = MenuItem::with_id(app, "ctx:rebuild", "重建应用索引", true, None::<&str>)?;
    actions.insert("ctx:rebuild".to_string(), ContextMenuAction::RebuildIndex);
    menu.append(&rebuild_mi)?;

    let settings_mi = MenuItem::with_id(app, "ctx:settings", "偏好设置", true, None::<&str>)?;
    actions.insert("ctx:settings".to_string(), ContextMenuAction::OpenSettings);
    menu.append(&settings_mi)?;

    popup_on_main_window(app, &state, &menu, actions)
}

pub fn popup_brand_context_menu(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let in_settings = state.settings_open();
    let mut actions = HashMap::new();
    let menu = Menu::new(app)?;

    let settings_label = if in_settings {
        "返回搜索"
    } else {
        "打开设置"
    };
    let settings_mi =
        MenuItem::with_id(app, "ctx:brand-settings", settings_label, true, None::<&str>)?;
    actions.insert(
        "ctx:brand-settings".to_string(),
        ContextMenuAction::OpenSettings,
    );
    menu.append(&settings_mi)?;

    let dash_mi = MenuItem::with_id(
        app,
        "ctx:brand-dashboard",
        "展开 / 收起所有应用",
        true,
        None::<&str>,
    )?;
    actions.insert(
        "ctx:brand-dashboard".to_string(),
        ContextMenuAction::ToggleDashboard,
    );
    menu.append(&dash_mi)?;

    let rebuild_mi =
        MenuItem::with_id(app, "ctx:brand-rebuild", "重建应用索引", true, None::<&str>)?;
    actions.insert("ctx:brand-rebuild".to_string(), ContextMenuAction::RebuildIndex);
    menu.append(&rebuild_mi)?;

    let sep = PredefinedMenuItem::separator(app)?;
    menu.append(&sep)?;

    let quit_mi = MenuItem::with_id(app, "ctx:brand-quit", "退出 Triex", true, None::<&str>)?;
    actions.insert("ctx:brand-quit".to_string(), ContextMenuAction::QuitApp);
    menu.append(&quit_mi)?;

    popup_on_main_window(app, &state, &menu, actions)
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
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = launch_item_by_id(&handle, &item_id, as_admin) {
                    let _ = handle.emit("triex://status-message", format!("启动失败: {err}"));
                }
            });
        }
        ContextMenuAction::TogglePin { item_id } => {
            let was_pinned = state
                .current_settings()
                .pinned_items
                .iter()
                .any(|id| id == &item_id);
            match toggle_pin(app, item_id) {
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
            if remove_recent_by_id(app, &item_id).is_ok() {
                let _ = app.emit("triex://status-message", "已从最近使用移除");
            }
        }
        ContextMenuAction::OpenFolder { item_id } => {
            if let Err(err) = open_folder_by_id(app, &item_id) {
                let _ = app.emit("triex://status-message", format!("打开位置失败: {err}"));
            }
        }
        ContextMenuAction::ToggleDashboard => {
            close_settings_view(app);
            let _ = app.emit("triex://toggle-dashboard", ());
        }
        ContextMenuAction::RebuildIndex => {
            let handle = app.clone();
            let _ = app.emit("triex://status-message", "正在重建索引...");
            tauri::async_runtime::spawn(async move {
                if let Ok(status) = trigger_rebuild_index(&handle).await {
                    let _ = handle.emit(
                        "triex://status-message",
                        format!("已更新 {} 项", status.total_indexed),
                    );
                }
            });
        }
        ContextMenuAction::OpenSettings => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if handle.state::<AppState>().settings_open() {
                    close_settings_view(&handle);
                } else {
                    let _ = open_settings_window(&handle);
                }
            });
        }
        ContextMenuAction::QuitApp => {
            app.exit(0);
        }
    }
}
