use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::launcher::{show_main_launcher, toggle_main_launcher};
use crate::config::AppSettings;
use crate::error::{AppError, AppResult};
use crate::platform::window::apply_window_vibrancy;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.current_settings()
}

#[tauri::command]
pub async fn update_settings(
    new_settings: AppSettings,
    app: AppHandle,
) -> AppResult<AppSettings> {
    let state = app.state::<AppState>();
    let old_settings = state.current_settings();

    if new_settings.hotkey != old_settings.hotkey {
        register_launcher_hotkey(&app, &new_settings.hotkey)?;
    }

    if new_settings.launch_on_startup != old_settings.launch_on_startup {
        let autostart = app.autolaunch();
        let _ = if new_settings.launch_on_startup {
            autostart.enable()
        } else {
            autostart.disable()
        };
    }

    if (new_settings.vibrancy != old_settings.vibrancy || new_settings.theme != old_settings.theme)
        && let Some(main_win) = app.get_webview_window("main")
    {
        apply_window_vibrancy(&main_win, new_settings.vibrancy, new_settings.theme);
    }

    state.db.save_settings(&new_settings)?;
    if let Ok(mut guard) = state.settings.write() {
        *guard = new_settings.clone();
    }

    let needs_reindex = new_settings.custom_scan_dirs != old_settings.custom_scan_dirs
        || new_settings.excluded_apps != old_settings.excluded_apps;
    if needs_reindex {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = crate::commands::search::trigger_rebuild_index(&handle).await;
        });
    }

    let _ = app.emit("triex://settings-updated", &new_settings);
    Ok(new_settings)
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> AppResult<()> {
    open_settings_window(&app)
}

/// Shows the main launcher window and switches it into the embedded ZTools-style Settings view.
pub fn open_settings_window(app: &AppHandle) -> AppResult<()> {
    show_main_launcher(app)?;
    let _ = app.emit("triex://open-settings", ());
    Ok(())
}

/// Unregisters previous global shortcuts and registers the configured launcher toggle hotkey.
pub fn register_launcher_hotkey(app: &AppHandle, hotkey: &str) -> AppResult<()> {
    let parsed: Shortcut = hotkey
        .parse()
        .map_err(|e| AppError::InvalidInput(format!("Invalid hotkey '{hotkey}': {e}")))?;

    let manager = app.global_shortcut();
    let _ = manager.unregister_all();

    manager
        .on_shortcut(parsed, |app_handle, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = toggle_main_launcher(app_handle);
            }
        })
        .map_err(|e| AppError::Platform(format!("Failed to register hotkey '{hotkey}': {e}")))?;

    Ok(())
}
