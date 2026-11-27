use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Webview};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_updater::UpdaterExt;

use crate::commands::settings::destroy_settings_webview;
use crate::error::{AppError, AppResult};
use crate::platform::launch::open_directory;
use crate::plugins::{manager::close_all_cached_plugin_webviews, permission::require_host_webview};
use crate::state::AppState;

/// Serializable summary of an available update returned to the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

/// Download progress payload emitted on `triex://update-progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub content_length: Option<u64>,
}

/// Checks the configured updater endpoint and caches the pending `Update` handle on success.
pub async fn check_for_update_inner(app: &AppHandle) -> AppResult<Option<UpdateMetadata>> {
    let updater = app
        .updater()
        .map_err(|err| AppError::Platform(format!("无法初始化更新器: {err}")))?;

    let update = updater
        .check()
        .await
        .map_err(|err| AppError::Platform(format!("检查更新失败: {err}")))?;

    let state = app.state::<AppState>();

    if let Some(update) = update {
        let meta = UpdateMetadata {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            date: update.date.map(|d| d.to_string()),
            body: update.body.clone(),
        };

        *state.pending_update.lock().await = Some(update);
        if let Ok(mut cached) = state.cached_update_meta.lock() {
            *cached = Some(meta.clone());
        }

        let _ = app.emit("triex://update-available", &meta);
        Ok(Some(meta))
    } else {
        *state.pending_update.lock().await = None;
        if let Ok(mut cached) = state.cached_update_meta.lock() {
            *cached = None;
        }
        Ok(None)
    }
}

#[tauri::command]
pub async fn check_update(webview: Webview, app: AppHandle) -> AppResult<Option<UpdateMetadata>> {
    require_host_webview(&webview)?;
    check_for_update_inner(&app).await
}

#[tauri::command]
pub fn get_cached_update(
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<Option<UpdateMetadata>> {
    require_host_webview(&webview)?;
    Ok(state
        .cached_update_meta
        .lock()
        .ok()
        .and_then(|guard| guard.clone()))
}

#[tauri::command]
pub async fn install_update(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;

    let state = app.state::<AppState>();
    let update = state
        .pending_update
        .lock()
        .await
        .take()
        .ok_or_else(|| AppError::NotFound("没有可安装的更新，请先检查更新".to_string()))?;

    let progress_app = app.clone();
    let cleanup_app = app.clone();
    let mut downloaded: u64 = 0;

    update
        .download_and_install(
            move |chunk_len, content_length| {
                downloaded = downloaded.saturating_add(chunk_len as u64);
                let _ = progress_app.emit(
                    "triex://update-progress",
                    UpdateProgress {
                        downloaded,
                        content_length,
                    },
                );
            },
            move || {
                cleanup_before_update_exit(&cleanup_app);
            },
        )
        .await
        .map_err(|err| AppError::Platform(format!("下载或安装更新失败: {err}")))?;

    Ok(())
}

/// Releases global hotkeys, directory watchers, and kept-alive child webviews before
/// the NSIS installer replaces the application binaries.
fn cleanup_before_update_exit(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();

    let state = app.state::<AppState>();
    if let Ok(mut watcher) = state.dir_watcher.lock() {
        *watcher = None;
    }

    close_all_cached_plugin_webviews(app);
    destroy_settings_webview(app);
}

#[tauri::command]
pub fn open_data_dir(webview: Webview, state: State<'_, AppState>) -> AppResult<()> {
    require_host_webview(&webview)?;
    open_directory(&state.paths.data_dir)
}
