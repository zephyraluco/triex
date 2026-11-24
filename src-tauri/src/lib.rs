pub mod commands;
pub mod config;
pub mod error;
pub mod platform;
pub mod plugins;
pub mod protocol;
pub mod search;
pub mod state;
pub mod storage;
pub mod tray;

use std::sync::atomic::Ordering;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tracing_subscriber::EnvFilter;

use crate::commands::launcher::{
    handle_context_menu_event, hide_main_launcher, show_main_launcher,
};
use crate::commands::search::trigger_rebuild_index;
use crate::commands::settings::register_launcher_hotkey;
use crate::config::AppPaths;
use crate::platform::scanner::watcher::spawn_directory_watcher;
use crate::platform::scanner::win32::start_menu_directories;
use crate::platform::window::{apply_window_vibrancy, position_on_cursor_monitor};
use crate::protocol::icon::handle_icon_protocol;
use crate::protocol::plugin::handle_plugin_protocol;
use crate::state::AppState;
use crate::storage::Database;
use crate::tray::setup_tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_main_launcher(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--silent"]),
        ))
        .register_uri_scheme_protocol("icon", |ctx, request| {
            handle_icon_protocol(ctx.app_handle(), request)
        })
        .register_uri_scheme_protocol("plugin", |ctx, request| {
            handle_plugin_protocol(ctx.app_handle(), request)
        })
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("triex"));
            let paths = AppPaths::resolve(data_dir)?;
            let db = Database::open(&paths.db_path)?;
            let settings = db.load_settings()?;

            // Populate in-memory search index immediately from SQLite cache for instant startup.
            let state = AppState::new(paths, db, settings.clone());
            if let Ok(cached_items) = state.db.load_cached_items()
                && !cached_items.is_empty()
            {
                let usage = state.db.load_all_usage().unwrap_or_default();
                if let Ok(mut engine) = state.search.lock() {
                    engine.rebuild(cached_items, &usage, &settings.excluded_apps);
                }
            }
            app.manage(state);

            // Configure main launcher window visuals and focus behavior.
            if let Some(main_win) = app.get_webview_window("main") {
                apply_window_vibrancy(&main_win, settings.vibrancy, settings.theme);
                position_on_cursor_monitor(&main_win);

                let menu_handle = app.handle().clone();
                main_win.on_menu_event(move |_window, event| {
                    let id = event.id().as_ref();
                    if id.starts_with("ctx:") {
                        handle_context_menu_event(&menu_handle, id);
                    }
                });

                let handle = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let st = handle.state::<AppState>();
                        let has_active_plugin = st
                            .active_plugin
                            .lock()
                            .map(|g| g.is_some())
                            .unwrap_or(false);
                        let menu_open = st.context_menu_open.load(Ordering::SeqCst);
                        let in_settings = st.in_settings_view.load(Ordering::SeqCst);
                        // Avoid closing when focus moves into a child plugin webview, native popup menu, or embedded settings.
                        if st.current_settings().hide_on_blur
                            && !has_active_plugin
                            && !menu_open
                            && !in_settings
                        {
                            hide_main_launcher(&handle, false);
                        }
                    }
                });
            }

            if let Err(err) = setup_tray(app.handle()) {
                tracing::warn!("Failed to initialize system tray: {err}");
            }

            if let Err(err) = register_launcher_hotkey(app.handle(), &settings.hotkey) {
                tracing::warn!("Failed to register global hotkey '{}': {err}", settings.hotkey);
            }

            // Trigger full asynchronous application & UWP scan in background.
            let scan_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = trigger_rebuild_index(&scan_handle).await {
                    tracing::error!("Background app scan failed: {err}");
                }
            });

            // Watch Start Menu directories for newly installed/removed apps.
            let watch_dirs = start_menu_directories(&settings.custom_scan_dirs);
            let watch_handle = app.handle().clone();
            let watcher = spawn_directory_watcher(watch_dirs, move || {
                let h = watch_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = trigger_rebuild_index(&h).await;
                });
            });
            if let Ok(mut guard) = app.state::<AppState>().dir_watcher.lock() {
                *guard = watcher;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_items,
            commands::search::rebuild_index,
            commands::launcher::launch_item,
            commands::launcher::open_item_folder,
            commands::launcher::toggle_pin_item,
            commands::launcher::remove_recent_item,
            commands::launcher::show_item_context_menu,
            commands::launcher::show_brand_context_menu,
            commands::launcher::set_settings_view_open,
            commands::launcher::resize_window,
            commands::launcher::hide_window,
            commands::launcher::complete_hide,
            commands::clipboard::get_clipboard_snapshot,
            commands::clipboard::paste_to_previous_window,
            commands::plugin::list_plugins,
            commands::plugin::set_plugin_enabled,
            commands::plugin::open_plugin_command,
            commands::plugin::close_plugin_command,
            commands::plugin::detach_plugin_command,
            commands::plugin::set_plugin_sub_input,
            commands::plugin::forward_sub_input_value,
            commands::plugin::plugin_db_get,
            commands::plugin::plugin_db_put,
            commands::plugin::plugin_db_delete,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running triex application");
}
