use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::commands::{everything, settings::close_settings_view};
use crate::error::{AppError, AppResult};
use crate::platform::window::{set_logical_height, set_shell_webview_bounds};
use crate::plugins::EVERYTHING_ID;
use crate::state::{AppState, CachedPluginWebview};

const PLUGIN_BRIDGE_JS: &str = include_str!("../../resources/plugin-bridge.js");
const LAUNCHER_WIDTH: f64 = 800.0;
const HEADER_HEIGHT: f64 = 64.0;
const PLUGIN_KEEP_ALIVE_SECS: u64 = 60;
static PLUGIN_VIEW_SEQ: AtomicU64 = AtomicU64::new(1);
static PLUGIN_EVICT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn build_plugin_url(plugin_id: &str, entry: &str) -> AppResult<tauri::Url> {
    let entry = entry.trim_start_matches('/');
    #[cfg(target_os = "windows")]
    let url_str = format!("http://plugin.localhost/{plugin_id}/{entry}");
    #[cfg(not(target_os = "windows"))]
    let url_str = format!("plugin://{plugin_id}/{entry}");

    url_str
        .parse()
        .map_err(|e| AppError::Plugin(format!("Invalid plugin URL '{url_str}': {e}")))
}

fn build_plugin_init_script(
    theme: crate::config::ThemeMode,
    feature_code: &str,
    builtin: bool,
) -> String {
    let theme_str = match theme {
        crate::config::ThemeMode::Light => "light",
        crate::config::ThemeMode::Dark => "dark",
        crate::config::ThemeMode::System => "system",
    };
    let code_json = serde_json::to_string(feature_code).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "window.__TRIEX_THEME__ = '{theme_str}';\nwindow.__TRIEX_FEATURE_CODE__ = {code_json};\nwindow.__TRIEX_BUILTIN_EVERYTHING__ = {builtin};\n{PLUGIN_BRIDGE_JS}"
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePluginSession {
    pub plugin_id: String,
    pub plugin_name: String,
    pub feature_code: String,
    pub webview_label: String,
    pub sub_input_placeholder: String,
}

/// Mounts a plugin UI as a sandboxed child webview inside the main launcher window.
pub fn open_plugin(
    app: &AppHandle,
    plugin_id: &str,
    feature_code: &str,
) -> AppResult<ActivePluginSession> {
    let state = app.state::<AppState>();
    let plugins = state.db.list_plugins()?;
    let plugin = plugins
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled)
        .ok_or_else(|| AppError::NotFound(format!("Plugin '{plugin_id}' not found or disabled")))?;

    close_settings_view(app);
    close_active_plugin(app)?;

    let main_window = app
        .get_window("main")
        .ok_or_else(|| AppError::Platform("Main window not available".to_string()))?;

    let builtin = plugin.id == EVERYTHING_ID;
    let plugin_height = plugin.manifest.height.clamp(240.0, 720.0);
    let reused_label = state
        .cached_plugin_webviews
        .lock()
        .ok()
        .and_then(|mut map| {
            let entry = map.get_mut(&plugin.id)?;
            if app.get_webview(&entry.label).is_some() {
                // Bump epoch to cancel any pending idle-eviction timer for this plugin.
                entry.epoch = PLUGIN_EVICT_EPOCH.fetch_add(1, Ordering::Relaxed);
                Some(entry.label.clone())
            } else {
                map.remove(&plugin.id);
                None
            }
        });

    let label = if let Some(existing_label) = reused_label {
        if let Some(wv) = app.get_webview(&existing_label) {
            let _ = wv.set_position(LogicalPosition::new(0.0, HEADER_HEIGHT));
            let _ = wv.set_size(LogicalSize::new(LAUNCHER_WIDTH, plugin_height));
            let _ = wv.show();
            let _ = wv.set_focus();
            if let Ok(code_json) = serde_json::to_string(feature_code) {
                let script = format!(
                    "window.dispatchEvent(new CustomEvent('triex:plugin-enter', {{ detail: {{ code: {code_json} }} }}));"
                );
                let _ = wv.eval(&script);
            }
        }
        existing_label
    } else {
        let seq = PLUGIN_VIEW_SEQ.fetch_add(1, Ordering::Relaxed);
        let (new_label, url) = if builtin {
            (
                format!("{}{seq}", everything::EMBEDDED_PREFIX),
                WebviewUrl::App(everything::ENTRY.into()),
            )
        } else {
            (
                format!("plugin-{}:{seq}", plugin.id),
                WebviewUrl::External(build_plugin_url(&plugin.id, &plugin.manifest.entry)?),
            )
        };
        let init_script =
            build_plugin_init_script(state.current_settings().theme, feature_code, builtin);

        let handle = app.clone();
        let builder = WebviewBuilder::new(&new_label, url)
            .on_navigation(move |url| !builtin || everything::is_builtin_url(&handle, url))
            .on_page_load(move |view, payload| {
                if builtin && matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                    everything::clear_results(view.app_handle(), view.label());
                }
            })
            .transparent(true)
            .initialization_script(&init_script);

        let _child = main_window
            .add_child(
                builder,
                LogicalPosition::new(0.0, HEADER_HEIGHT),
                LogicalSize::new(LAUNCHER_WIDTH, plugin_height),
            )
            .map_err(|err| {
                tracing::error!("Failed to add child plugin webview '{new_label}': {err}");
                AppError::Tauri(err)
            })?;

        let epoch = PLUGIN_EVICT_EPOCH.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = state.cached_plugin_webviews.lock() {
            map.insert(
                plugin.id.clone(),
                CachedPluginWebview {
                    label: new_label.clone(),
                    epoch,
                },
            );
        }
        new_label
    };

    // Shrink the primary shell webview to header-only so it never overlaps the plugin webview.
    set_shell_webview_bounds(app, LAUNCHER_WIDTH, HEADER_HEIGHT);
    set_logical_height(&main_window, LAUNCHER_WIDTH, HEADER_HEIGHT + plugin_height);

    let session = ActivePluginSession {
        plugin_id: plugin.id.clone(),
        plugin_name: plugin.name.clone(),
        feature_code: feature_code.to_string(),
        webview_label: label,
        sub_input_placeholder: if builtin {
            "输入文件名或 Everything 查询".into()
        } else {
            format!("在 {} 中输入...", plugin.name)
        },
    };

    *state.content_mode.lock().map_err(|_| AppError::Platform("Content mode lock poisoned".into()))? = crate::state::ContentMode::Plugin(session.clone());

    let _ = app.emit("triex://plugin-opened", &session);
    Ok(session)
}

/// Immediately destroys a cached plugin child webview (e.g. when detaching or disabling).
pub fn evict_plugin_webview(app: &AppHandle, plugin_id: &str) {
    let state = app.state::<AppState>();
    let removed = state
        .cached_plugin_webviews
        .lock()
        .ok()
        .and_then(|mut map| map.remove(plugin_id));

    if let Some(entry) = removed
        && let Some(wv) = app.get_webview(&entry.label)
    {
        let _ = wv.close();
    }
}

/// Closes all kept-alive plugin child webviews (used during graceful shutdown before update install).
pub fn close_all_cached_plugin_webviews(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = state.take_active_plugin();
    let labels: Vec<String> = state
        .cached_plugin_webviews
        .lock()
        .ok()
        .map(|mut map| map.drain().map(|(_, entry)| entry.label).collect())
        .unwrap_or_default();

    for label in labels {
        if let Some(wv) = app.get_webview(&label) {
            let _ = wv.close();
        }
    }
}

/// Hides any currently embedded child plugin webview (keeping it warm for fast reopen,
/// then automatically closing it after `PLUGIN_KEEP_ALIVE_SECS` of inactivity).
pub fn close_active_plugin(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let existing = state.take_active_plugin();

    if let Some(session) = existing {
        if session.plugin_id == EVERYTHING_ID {
            everything::clear_results(app, &session.webview_label);
            evict_plugin_webview(app, EVERYTHING_ID);
            if !state.settings_open()
                && let Some(shell) = app.get_webview("main")
            {
                let _ = shell.set_focus();
            }
            let _ = app.emit("triex://plugin-closed", &session.plugin_id);
            return Ok(());
        }
        if let Some(wv) = app.get_webview(&session.webview_label) {
            let _ = wv.hide();
        }

        let evict_epoch = PLUGIN_EVICT_EPOCH.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = state.cached_plugin_webviews.lock() {
            map.insert(
                session.plugin_id.clone(),
                CachedPluginWebview {
                    label: session.webview_label.clone(),
                    epoch: evict_epoch,
                },
            );
        }

        let handle = app.clone();
        let plugin_id = session.plugin_id.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(PLUGIN_KEEP_ALIVE_SECS)).await;
            let st = handle.state::<AppState>();
            let is_active = st.active_plugin().is_some_and(|session| session.plugin_id == plugin_id);
            if is_active {
                return;
            }
            let stale_label = st.cached_plugin_webviews.lock().ok().and_then(|mut map| {
                if map.get(&plugin_id).is_some_and(|e| e.epoch == evict_epoch) {
                    map.remove(&plugin_id).map(|e| e.label)
                } else {
                    None
                }
            });
            if let Some(label) = stale_label
                && let Some(wv) = handle.get_webview(&label)
            {
                let _ = wv.close();
            }
        });

        if !state.settings_open()
            && let Some(shell_wv) = app.get_webview("main")
        {
            let _ = shell_wv.set_focus();
        }
        let _ = app.emit("triex://plugin-closed", &session.plugin_id);
    }

    Ok(())
}

/// Detaches the active plugin into its own independent OS window.
pub fn detach_active_plugin(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let existing = state.active_plugin()
        .ok_or_else(|| AppError::NotFound("No active plugin to detach".to_string()))?;

    let plugins = state.db.list_plugins()?;
    let plugin = plugins
        .into_iter()
        .find(|p| p.id == existing.plugin_id)
        .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found", existing.plugin_id)))?;

    close_settings_view(app);
    close_active_plugin(app)?;
    evict_plugin_webview(app, &plugin.id);

    let builtin = plugin.id == EVERYTHING_ID;
    let detached_label = if builtin {
        everything::DETACHED_LABEL.to_owned()
    } else {
        format!("plugin-detached-{}", plugin.id)
    };
    if let Some(win) = app.get_webview_window(&detached_label) {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let url = if builtin {
        everything::clear_results(app, everything::DETACHED_LABEL);
        WebviewUrl::App(everything::ENTRY.into())
    } else {
        WebviewUrl::External(build_plugin_url(&plugin.id, &plugin.manifest.entry)?)
    };
    let init_script =
        build_plugin_init_script(state.current_settings().theme, &existing.feature_code, builtin);

    let handle = app.clone();
    let window = WebviewWindowBuilder::new(app, &detached_label, url)
        .on_navigation(move |url| !builtin || everything::is_builtin_url(&handle, url))
        .on_page_load(move |view, payload| {
            if builtin && matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                everything::clear_results(view.app_handle(), view.label());
            }
        })
        .title(&plugin.name)
        .inner_size(LAUNCHER_WIDTH, plugin.manifest.height.clamp(360.0, 800.0))
        .initialization_script(&init_script)
        .center()
        .build()?;
    if builtin {
        let handle = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                everything::clear_results(&handle, everything::DETACHED_LABEL);
            }
        });
    }

    Ok(())
}
