use tauri::AppHandle;

use crate::commands::launcher::hide_main_launcher;
use crate::error::AppResult;
use crate::platform::clipboard::{current_clipboard_snapshot, ClipboardSnapshot};
use crate::platform::input::send_ctrl_v;

#[tauri::command]
pub fn get_clipboard_snapshot() -> Option<ClipboardSnapshot> {
    current_clipboard_snapshot()
}

#[tauri::command]
pub fn paste_to_previous_window(app: AppHandle) -> AppResult<()> {
    hide_main_launcher(&app, true);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(60));
        send_ctrl_v();
    });
    Ok(())
}
