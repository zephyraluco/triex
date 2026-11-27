use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde_json::Value;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, Webview};

use crate::commands::search::trigger_rebuild_index;
use crate::error::{AppError, AppResult};
use crate::platform::launch::reveal_in_explorer;
use crate::platform::window::set_logical_height;
use crate::plugins::backend::{exec_plugin_command, ExecCommandOutput};
use crate::plugins::{inspect_plugin_manifest, install_plugin_from_path};
use crate::plugins::manager::{
    close_active_plugin, detach_active_plugin, evict_plugin_webview, open_plugin,
    ActivePluginSession,
};
use crate::plugins::manifest::PluginManifest;
use crate::plugins::permission::{
    plugin_id_from_label, require_host_webview, require_permission,
};
use crate::state::AppState;
use crate::storage::plugin_repo::{InstalledPluginRecord, PluginKvEntry};

const LAUNCHER_WIDTH: f64 = 800.0;
const HEADER_HEIGHT: f64 = 64.0;

#[tauri::command]
pub fn list_plugins(
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<Vec<InstalledPluginRecord>> {
    require_host_webview(&webview)?;
    state.db.list_plugins()
}

/// Installs or updates a plugin from a `.txz` / `.zip` archive or local directory,
/// safely stopping any active webview or backend process for that plugin ID beforehand.
pub async fn install_plugin_source(app: &AppHandle, source_path: &str) -> AppResult<PluginManifest> {
    let trimmed = source_path.trim().trim_matches('"');
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("Plugin path is required".into()));
    }
    let path = PathBuf::from(trimmed);
    let preview = inspect_plugin_manifest(&path)?;

    let state = app.state::<AppState>();
    if state
        .active_plugin()
        .is_some_and(|session| session.plugin_id == preview.id)
    {
        close_active_plugin(app)?;
    }
    evict_plugin_webview(app, &preview.id);

    let manifest = install_plugin_from_path(&path, &state.paths, &state.db)?;
    let _ = trigger_rebuild_index(app).await?;
    Ok(manifest)
}

#[tauri::command]
pub async fn pick_and_install_plugin(
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<PluginManifest>> {
    require_host_webview(&webview)?;

    // Suppress hide-on-blur while the native OS file dialog is open.
    state.context_menu_open.store(true, Ordering::SeqCst);
    let owner_hwnd: isize = app
        .get_window("main")
        .and_then(|win| win.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0);

    let picked = tokio::task::spawn_blocking(move || open_plugin_file_dialog(owner_hwnd))
        .await
        .map_err(|err| AppError::Platform(format!("File dialog task failed: {err}")));

    state.context_menu_open.store(false, Ordering::SeqCst);
    let Some(selected_path) = picked?? else {
        return Ok(None);
    };

    let manifest = install_plugin_source(&app, &selected_path).await?;
    Ok(Some(manifest))
}

#[cfg(windows)]
fn open_plugin_file_dialog(owner_hwnd: isize) -> AppResult<Option<String>> {
    use windows::core::w;
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FileOpenDialog,
        IFileOpenDialog, SIGDN_FILESYSPATH,
    };

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = hr.is_ok();

        let dialog_res = (|| -> windows::core::Result<Option<String>> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetTitle(w!("选择 Triex 插件安装包"))?;
            dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM)?;

            let filters = [
                COMDLG_FILTERSPEC {
                    pszName: w!("Triex 插件安装包 (*.txz;*.zip)"),
                    pszSpec: w!("*.txz;*.zip"),
                },
                COMDLG_FILTERSPEC {
                    pszName: w!("所有文件 (*.*)"),
                    pszSpec: w!("*.*"),
                },
            ];
            dialog.SetFileTypes(&filters)?;
            dialog.SetFileTypeIndex(1)?;

            let owner = HWND(owner_hwnd as *mut std::ffi::c_void);
            if let Err(err) = dialog.Show(owner) {
                if err.code() == ERROR_CANCELLED.to_hresult() {
                    return Ok(None);
                }
                return Err(err);
            }

            let item = dialog.GetResult()?;
            let path_pwstr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = path_pwstr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(path_pwstr.0.cast()));

            if path_str.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path_str))
            }
        })();

        if initialized {
            CoUninitialize();
        }

        dialog_res.map_err(|err| AppError::Platform(format!("Failed to open file dialog: {err}")))
    }
}

#[cfg(not(windows))]
fn open_plugin_file_dialog(_owner_hwnd: isize) -> AppResult<Option<String>> {
    Err(AppError::Platform(
        "Native file dialog is only implemented for Windows".into(),
    ))
}

#[tauri::command]
pub async fn install_plugin_path(
    source_path: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<PluginManifest> {
    require_host_webview(&webview)?;
    install_plugin_source(&app, &source_path).await
}

#[tauri::command]
pub async fn install_plugin_dir(
    source_dir: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<PluginManifest> {
    require_host_webview(&webview)?;
    install_plugin_source(&app, &source_dir).await
}

#[tauri::command]
pub async fn uninstall_plugin(
    plugin_id: String,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    if state
        .active_plugin()
        .is_some_and(|session| session.plugin_id == plugin_id)
    {
        close_active_plugin(&app)?;
    }
    evict_plugin_webview(&app, &plugin_id);

    let plugin_dir = state.paths.plugin_dir(&plugin_id);
    if plugin_dir.exists() && crate::config::AppPaths::ensure_within_dir(&state.paths.plugins_dir, &plugin_dir) {
        std::fs::remove_dir_all(&plugin_dir)?;
    }
    state.db.remove_plugin(&plugin_id)?;
    let _ = trigger_rebuild_index(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn reload_plugins(
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Vec<InstalledPluginRecord>> {
    require_host_webview(&webview)?;
    let _ = trigger_rebuild_index(&app).await?;
    state.db.list_plugins()
}

#[tauri::command]
pub fn open_plugins_dir(webview: Webview, state: State<'_, AppState>) -> AppResult<()> {
    require_host_webview(&webview)?;
    std::fs::create_dir_all(&state.paths.plugins_dir)?;
    let dir_str = state.paths.plugins_dir.to_string_lossy().to_string();
    reveal_in_explorer(&dir_str)
}

#[tauri::command]
pub fn set_plugin_enabled(
    plugin_id: String,
    enabled: bool,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    state.db.set_plugin_enabled(&plugin_id, enabled)?;
    if !enabled {
        let is_active = state
            .active_plugin()
            .is_some_and(|session| session.plugin_id == plugin_id);
        if is_active {
            close_active_plugin(&app)?;
        }
        evict_plugin_webview(&app, &plugin_id);
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = trigger_rebuild_index(&handle).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn open_plugin_command(
    plugin_id: String,
    feature_code: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<ActivePluginSession> {
    require_host_webview(&webview)?;
    open_plugin(&app, &plugin_id, &feature_code)
}

#[tauri::command]
pub fn close_plugin_command(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    close_active_plugin(&app)
}

#[tauri::command]
pub async fn detach_plugin_command(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    detach_active_plugin(&app)
}

#[tauri::command]
pub fn set_plugin_sub_input(
    placeholder: String,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    let plugins = state.db.list_plugins()?;
    if !plugins
        .iter()
        .any(|plugin| plugin.id == plugin_id && plugin.enabled)
    {
        return Err(AppError::PermissionDenied("Plugin is disabled".into()));
    }
    let mut guard = state
        .content_mode
        .lock()
        .map_err(|_| AppError::Platform("Plugin lock poisoned".into()))?;
    let crate::state::ContentMode::Plugin(session) = &mut *guard else {
        return Err(AppError::PermissionDenied("No active plugin session".into()));
    };
    if session.webview_label != webview.label() {
        return Err(AppError::PermissionDenied(
            "Caller does not own the active session".into(),
        ));
    }
    session.sub_input_placeholder = placeholder.clone();
    drop(guard);
    let _ = app.emit("triex://sub-input-placeholder", placeholder);
    Ok(())
}

#[tauri::command]
pub fn plugin_set_height(
    height: f64,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let _plugin_id = plugin_id_from_label(webview.label())?;
    let is_owner = state
        .active_plugin()
        .is_some_and(|session| session.webview_label == webview.label());
    if !is_owner {
        return Err(AppError::PermissionDenied(
            "Caller does not own the active embedded plugin session".into(),
        ));
    }

    let clamped = height.clamp(240.0, 720.0);
    let _ = webview.set_size(LogicalSize::new(LAUNCHER_WIDTH, clamped));
    if let Some(main_window) = app.get_window("main") {
        set_logical_height(&main_window, LAUNCHER_WIDTH, HEADER_HEIGHT + clamped);
    }
    Ok(())
}

#[tauri::command]
pub async fn plugin_backend_call(
    method: String,
    params: Option<Value>,
    timeout_ms: Option<u64>,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Value> {
    let plugin_id = plugin_id_from_label(webview.label())?.to_string();
    require_permission(&state.db, &plugin_id, "backend")?;

    let plugin = state
        .db
        .list_plugins()?
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled)
        .ok_or_else(|| AppError::NotFound(format!("Enabled plugin '{plugin_id}' not found")))?;

    let backend_cfg = plugin.manifest.backend.ok_or_else(|| {
        AppError::Plugin(format!(
            "Plugin '{plugin_id}' does not declare a backend in plugin.json"
        ))
    })?;

    state
        .plugin_backends
        .call(
            &app,
            &plugin_id,
            &backend_cfg,
            &state.paths,
            &method,
            params.unwrap_or(Value::Null),
            timeout_ms,
        )
        .await
}

#[tauri::command]
pub async fn plugin_exec_command(
    command: String,
    args: Option<Vec<String>>,
    timeout_ms: Option<u64>,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<ExecCommandOutput> {
    let plugin_id = plugin_id_from_label(webview.label())?.to_string();
    require_permission(&state.db, &plugin_id, "process")?;
    let plugin_root = state.paths.plugin_dir(&plugin_id);
    exec_plugin_command(&plugin_root, command, args.unwrap_or_default(), timeout_ms).await
}

#[tauri::command]
pub fn forward_sub_input_value(
    value: String,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    let label = state.active_plugin().map(|s| s.webview_label);

    if let Some(label) = label
        && let Some(wv) = app.get_webview(&label)
    {
        let payload = serde_json::to_string(&value)?;
        let script = format!(
            "window.dispatchEvent(new CustomEvent('triex:sub-input-change', {{ detail: {payload} }}));"
        );
        let _ = wv.eval(&script);
    }
    Ok(())
}

#[tauri::command]
pub fn forward_sub_input_keydown(
    key: String,
    shift_key: bool,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    let label = state.active_plugin().map(|s| s.webview_label);

    if let Some(label) = label
        && let Some(wv) = app.get_webview(&label)
    {
        let detail = serde_json::json!({
            "key": key,
            "shiftKey": shift_key,
        });
        let script = format!(
            "window.dispatchEvent(new CustomEvent('triex:sub-input-keydown', {{ detail: {detail} }}));"
        );
        let _ = wv.eval(&script);
    }
    Ok(())
}

#[tauri::command]
pub fn plugin_db_get(
    key: String,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<Option<PluginKvEntry>> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_get(plugin_id, &key)
}

#[tauri::command]
pub fn plugin_db_put(
    key: String,
    value: serde_json::Value,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<PluginKvEntry> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_put(plugin_id, &key, &value)
}

#[tauri::command]
pub fn plugin_db_delete(
    key: String,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_delete(plugin_id, &key)
}
