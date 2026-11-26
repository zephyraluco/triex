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

use tauri::window::WindowBuilder;
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tracing_subscriber::EnvFilter;

use crate::commands::launcher::{hide_main_launcher, show_main_launcher};
use crate::commands::menu::handle_context_menu_event;
use crate::commands::search::{refresh_directory_watcher, trigger_rebuild_index};
use crate::commands::settings::register_launcher_hotkey;
use crate::commands::updater::check_for_update_inner;
use crate::config::AppPaths;
use crate::platform::window::{apply_window_vibrancy, is_window_or_child_foreground};
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
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
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

            // Create a pure top-level container Window hosting non-overlapping child WebViews:
            // `main` (header + search shell) and at most one below-header view (settings or plugin).
            // Always start hidden in the system tray (both autostart and manual launch) until invoked.
            let main_win = WindowBuilder::new(app, "main")
                .title("Triex")
                .inner_size(800.0, 64.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .center()
                .visible(false)
                .build()?;

            let shell_builder = WebviewBuilder::new("main", WebviewUrl::App("index.html".into()))
                .transparent(true);
            let _shell_wv = main_win.add_child(
                shell_builder,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(800.0, 64.0),
            )?;

            apply_window_vibrancy(&main_win, settings.vibrancy, settings.theme);

            let menu_handle = app.handle().clone();
            main_win.on_menu_event(move |_window, event| {
                let id = event.id().as_ref();
                if id.starts_with("ctx:") {
                    handle_context_menu_event(&menu_handle, id);
                }
            });

            let handle = app.handle().clone();
            main_win.on_window_event(move |event| {
                if let WindowEvent::ThemeChanged(theme) = event {
                    let settings = handle.state::<AppState>().current_settings();
                    if settings.theme == crate::config::ThemeMode::System
                        && let Some(window) = handle.get_window("main")
                    {
                        // Use the event's resolved preference; the window query may
                        // still reflect the previous theme while handling this event.
                        let resolved = match theme {
                            tauri::Theme::Dark => crate::config::ThemeMode::Dark,
                            _ => crate::config::ThemeMode::Light,
                        };
                        apply_window_vibrancy(&window, settings.vibrancy, resolved);
                    }
                }
                if let WindowEvent::Focused(false) = event {
                    let h = handle.clone();
                    // Debounce briefly so focus transitions between sibling child WebView2 HWNDs
                    // settle before checking whether OS foreground truly left Triex.
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(35)).await;
                        let st = h.state::<AppState>();
                        if !st.current_settings().hide_on_blur
                            || st.context_menu_open.load(Ordering::SeqCst)
                            || !st.is_shown.load(Ordering::SeqCst)
                        {
                            return;
                        }
                        if let Some(win) = h.get_window("main")
                            && !is_window_or_child_foreground(&win)
                        {
                            hide_main_launcher(&h, false);
                        }
                    });
                }
            });

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

            refresh_directory_watcher(app.handle())?;

            // Silently check for updates 15 seconds after startup without interrupting search.
            if settings.auto_check_update {
                let update_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    if !update_handle.state::<AppState>().current_settings().auto_check_update {
                        return;
                    }
                    if let Err(err) = check_for_update_inner(&update_handle).await {
                        tracing::debug!("Background update check skipped/failed: {err}");
                    }
                });
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
            commands::launcher::resize_window,
            commands::launcher::hide_window,
            commands::launcher::complete_hide,
            commands::clipboard::get_clipboard_snapshot,
            commands::clipboard::read_clipboard_text,
            commands::clipboard::paste_to_previous_window,
            commands::plugin::list_plugins,
            commands::plugin::set_plugin_enabled,
            commands::plugin::open_plugin_command,
            commands::plugin::close_plugin_command,
            commands::plugin::detach_plugin_command,
            commands::plugin::set_plugin_sub_input,
            commands::plugin::forward_sub_input_value,
            commands::plugin::forward_sub_input_keydown,
            commands::plugin::plugin_db_get,
            commands::plugin::plugin_db_put,
            commands::plugin::plugin_db_delete,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_settings,
            commands::settings::close_settings,
            commands::settings::toggle_settings,
            commands::updater::check_update,
            commands::updater::get_cached_update,
            commands::updater::install_update,
            commands::updater::open_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running triex application");
}
