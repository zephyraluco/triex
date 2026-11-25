use tauri::{AppHandle, Webview};

use crate::error::AppResult;
use crate::platform::clipboard::{
    current_clipboard_snapshot, read_clipboard_text as read_os_clipboard_text, ClipboardSnapshot,
};
use crate::platform::input::send_ctrl_v;
use crate::plugins::permission::require_host_webview;
use crate::commands::launcher::hide_main_launcher;

#[tauri::command]
pub fn get_clipboard_snapshot() -> Option<ClipboardSnapshot> {
    current_clipboard_snapshot()
}

#[tauri::command]
pub fn read_clipboard_text(webview: Webview) -> AppResult<Option<String>> {
    require_host_webview(&webview)?;
    Ok(read_os_clipboard_text())
}

#[tauri::command]
pub fn paste_to_previous_window(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    hide_main_launcher(&app, true);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(60));
        send_ctrl_v();
    });
    Ok(())
}
