use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::commands::launcher::{show_main_launcher, toggle_main_launcher};
use crate::commands::search::trigger_rebuild_index;
use crate::commands::settings::open_settings_window;
use crate::error::AppResult;

pub fn setup_tray(app: &AppHandle) -> AppResult<()> {
    let show_item = MenuItem::with_id(app, "show", "显示启动器 (Alt+Space)", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "偏好设置...", true, None::<&str>)?;
    let rebuild_item = MenuItem::with_id(app, "rebuild", "重建应用索引", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 Triex", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &settings_item, &rebuild_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Triex 启动器")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                let _ = show_main_launcher(app);
            }
            "settings" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = open_settings_window(&handle);
                });
            }
            "rebuild" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = trigger_rebuild_index(&handle).await;
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_main_launcher(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}
