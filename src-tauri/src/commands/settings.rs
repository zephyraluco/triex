use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State,
    Webview, WebviewBuilder, WebviewUrl,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::launcher::{
    show_main_launcher, toggle_main_launcher, HEADER_LOGICAL_HEIGHT, LAUNCHER_LOGICAL_WIDTH,
};
use crate::commands::search::{refresh_directory_watcher, trigger_rebuild_index};
use crate::config::{AppSettings, SettingsPatch};
use crate::error::{AppError, AppResult};
use crate::platform::window::{
    apply_window_vibrancy, set_logical_height, set_shell_webview_bounds,
};
use crate::plugins::{manager::close_active_plugin, permission::require_host_webview};
use crate::state::AppState;

const SETTINGS_VIEW_HEIGHT: f64 = 516.0;
static SETTINGS_VIEW_SEQ: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
pub fn get_settings(webview: Webview, state: State<'_, AppState>) -> AppResult<AppSettings> {
    require_host_webview(&webview)?;
    Ok(state.current_settings())
}

#[tauri::command]
pub async fn update_settings(
    patch: SettingsPatch,
    webview: Webview,
    app: AppHandle,
) -> AppResult<AppSettings> {
    require_host_webview(&webview)?;
    apply_settings_change(&app, |settings| patch.apply(settings))
}

#[tauri::command]
pub async fn open_settings(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    open_settings_window(&app)
}

#[tauri::command]
pub async fn close_settings(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    close_settings_view(&app);
    Ok(())
}

#[tauri::command]
pub async fn toggle_settings(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    if app.state::<AppState>().settings_open() {
        close_settings_view(&app);
        Ok(())
    } else {
        open_settings_window(&app)
    }
}

pub fn toggle_pin(app: &AppHandle, item_id: String) -> AppResult<Vec<String>> {
    Ok(apply_settings_change(app, |settings| {
        if let Some(pos) = settings.pinned_items.iter().position(|id| id == &item_id) {
            settings.pinned_items.remove(pos);
        } else {
            settings.pinned_items.push(item_id);
        }
        Ok(())
    })?
    .pinned_items)
}

/// Serializes all settings writes (including pin changes) with rollback on hotkey/autostart failure.
fn apply_settings_change(
    app: &AppHandle,
    mutate: impl FnOnce(&mut AppSettings) -> AppResult<()>,
) -> AppResult<AppSettings> {
    let state = app.state::<AppState>();
    let _serial = state
        .settings_update
        .lock()
        .map_err(|_| AppError::Platform("Settings update lock poisoned".into()))?;
    let mut current = state
        .settings
        .write()
        .map_err(|_| AppError::Platform("Settings lock poisoned".into()))?;
    let old = current.clone();
    let mut next = old.clone();
    mutate(&mut next)?;

    let hotkey_changed = next.hotkey != old.hotkey;
    let startup_changed = next.launch_on_startup != old.launch_on_startup;

    // Register the replacement before removing the working shortcut.
    if hotkey_changed {
        register_launcher_hotkey(app, &next.hotkey)?;
    }

    let persist_res = (|| -> AppResult<()> {
        if startup_changed {
            let manager = app.autolaunch();
            (if next.launch_on_startup {
                manager.enable()
            } else {
                manager.disable()
            })
            .map_err(|err| AppError::Platform(format!("Cannot update autostart: {err}")))?;
        }
        state.db.save_settings(&next)
    })();

    if let Err(err) = persist_res {
        if hotkey_changed {
            let _ = app.global_shortcut().unregister(next.hotkey.as_str());
        }
        if startup_changed {
            let manager = app.autolaunch();
            let restored = if old.launch_on_startup {
                manager.enable()
            } else {
                manager.disable()
            };
            if let Err(rollback) = restored {
                return Err(AppError::Platform(format!(
                    "{err}; autostart rollback failed: {rollback}"
                )));
            }
        }
        return Err(err);
    }

    *current = next.clone();
    drop(current);

    if hotkey_changed
        && let Err(err) = app.global_shortcut().unregister(old.hotkey.as_str())
    {
        tracing::warn!("Failed to unregister previous shortcut: {err}");
    }
    if (next.theme != old.theme || next.vibrancy != old.vibrancy)
        && let Some(window) = app.get_window("main")
    {
        apply_window_vibrancy(&window, next.vibrancy, next.theme);
    }
    if next.custom_scan_dirs != old.custom_scan_dirs
        && let Err(err) = refresh_directory_watcher(app)
    {
        tracing::error!("Watcher refresh failed: {err}");
    }
    if next.custom_scan_dirs != old.custom_scan_dirs || next.excluded_apps != old.excluded_apps {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = trigger_rebuild_index(&handle).await {
                tracing::error!("Settings index refresh failed: {err}");
            }
        });
    }
    if let Err(err) = app.emit("triex://settings-updated", &next) {
        tracing::error!("Settings notification failed: {err}");
    }
    Ok(next)
}

/// Registers a shortcut without removing the currently working binding.
pub fn register_launcher_hotkey(app: &AppHandle, hotkey: &str) -> AppResult<()> {
    let parsed: Shortcut = hotkey
        .parse()
        .map_err(|e| AppError::InvalidInput(format!("Invalid hotkey '{hotkey}': {e}")))?;

    app.global_shortcut()
        .on_shortcut(parsed, |app_handle, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = toggle_main_launcher(app_handle);
            }
        })
        .map_err(|e| AppError::Platform(format!("Failed to register hotkey '{hotkey}': {e}")))?;

    Ok(())
}

/// Hides the settings child webview (keeping it alive for instant reopen)
/// and notifies the main header to return to search mode.
pub fn close_settings_view(app: &AppHandle) {
    let state = app.state::<AppState>();
    let was_open = state.set_settings_open(false);
    let existing_label = state
        .settings_webview_label
        .lock()
        .ok()
        .and_then(|guard| guard.clone());

    if let Some(label) = existing_label
        && let Some(wv) = app.get_webview(&label)
    {
        let _ = wv.hide();
    }

    if was_open {
        if let Some(shell_wv) = app.get_webview("main") {
            let _ = shell_wv.set_focus();
        }
        let _ = app.emit("triex://settings-closed", ());
    }
}

/// Permanently closes the kept-alive settings child webview (used before installer exit).
pub fn destroy_settings_webview(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.set_settings_open(false);
    let label = state
        .settings_webview_label
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    if let Some(label) = label
        && let Some(wv) = app.get_webview(&label)
    {
        let _ = wv.close();
    }
}

/// Shows the main launcher window and mounts (or reveals the kept-alive) Settings child webview
/// below the 64px header. Automatically hides any active plugin child webview first.
pub fn open_settings_window(app: &AppHandle) -> AppResult<()> {
    close_active_plugin(app)?;
    show_main_launcher(app)?;

    let state = app.state::<AppState>();
    let main_window = app
        .get_window("main")
        .ok_or_else(|| AppError::Platform("Main window not available".to_string()))?;

    let cached_wv = state
        .settings_webview_label
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .and_then(|label| app.get_webview(&label));

    let is_loading = if let Some(wv) = cached_wv {
        let _ = wv.set_position(LogicalPosition::new(0.0, HEADER_LOGICAL_HEIGHT));
        let _ = wv.set_size(LogicalSize::new(
            LAUNCHER_LOGICAL_WIDTH,
            SETTINGS_VIEW_HEIGHT,
        ));
        let _ = wv.show();
        let _ = wv.set_focus();
        false
    } else {
        let seq = SETTINGS_VIEW_SEQ.fetch_add(1, Ordering::Relaxed);
        let label = format!("settings-view:{seq}");
        let app_for_load = app.clone();
        let builder = WebviewBuilder::new(&label, WebviewUrl::App("settings.html".into()))
            .transparent(true)
            .on_page_load(move |_wv, payload| {
                if matches!(payload.event(), PageLoadEvent::Finished) {
                    let _ = app_for_load.emit("triex://view-loaded", "settings");
                }
            });

        main_window
            .add_child(
                builder,
                LogicalPosition::new(0.0, HEADER_LOGICAL_HEIGHT),
                LogicalSize::new(LAUNCHER_LOGICAL_WIDTH, SETTINGS_VIEW_HEIGHT),
            )
            .map_err(|err| {
                tracing::error!("Failed to add settings child webview '{label}': {err}");
                AppError::Tauri(err)
            })?;

        if let Ok(mut guard) = state.settings_webview_label.lock() {
            *guard = Some(label);
        }
        true
    };

    set_shell_webview_bounds(app, LAUNCHER_LOGICAL_WIDTH, HEADER_LOGICAL_HEIGHT);
    set_logical_height(
        &main_window,
        LAUNCHER_LOGICAL_WIDTH,
        HEADER_LOGICAL_HEIGHT + SETTINGS_VIEW_HEIGHT,
    );

    state.set_settings_open(true);
    let _ = app.emit("triex://settings-opened", is_loading);
    Ok(())
}
