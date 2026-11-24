use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::config::{ThemeMode, VibrancyMode};

/// Applies Windows 11 native DWM rounded corners and Mica / Acrylic vibrancy to the launcher window.
pub fn apply_window_vibrancy(window: &WebviewWindow, mode: VibrancyMode, theme: ThemeMode) {
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::{apply_acrylic, apply_mica, clear_acrylic, clear_mica};

        apply_win11_rounded_corners(window);

        let _ = clear_mica(window);
        let _ = clear_acrylic(window);

        let dark_hint = match theme {
            ThemeMode::Dark => Some(true),
            ThemeMode::Light => Some(false),
            ThemeMode::System => None,
        };

        match mode {
            VibrancyMode::Mica => {
                if apply_mica(window, dark_hint).is_err() {
                    // Fallback to acrylic on Windows 10 where Mica is unavailable.
                    let _ = apply_acrylic(window, Some((24, 24, 27, 210)));
                }
            }
            VibrancyMode::Acrylic => {
                let tint = match theme {
                    ThemeMode::Light => Some((248, 248, 250, 215)),
                    _ => Some((24, 24, 27, 215)),
                };
                let _ = apply_acrylic(window, tint);
            }
            VibrancyMode::None => {}
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, mode, theme);
    }
}

/// Explicitly enables Windows 11 DWM native rounded corners (`DWMWCP_ROUND`) so
/// borderless transparent windows have crisp hardware-antialiased corners without CSS double-rounding.
#[cfg(target_os = "windows")]
fn apply_win11_rounded_corners(window: &WebviewWindow) {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    if let Ok(raw_hwnd) = window.hwnd() {
        let hwnd = HWND(raw_hwnd.0);
        let preference = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::from_ref(&preference).cast::<c_void>(),
                std::mem::size_of_val(&preference) as u32,
            );
        }
    }
}

/// Captures the currently focused foreground OS window handle before showing the launcher.
#[cfg(windows)]
pub fn capture_foreground_hwnd() -> isize {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow().0 as isize }
}

#[cfg(not(windows))]
pub fn capture_foreground_hwnd() -> isize {
    0
}

/// Restores OS input focus to a previously captured window handle after hiding the launcher.
#[cfg(windows)]
pub fn restore_foreground_hwnd(hwnd_raw: isize) {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

    if hwnd_raw == 0 {
        return;
    }
    let hwnd = HWND(hwnd_raw as *mut c_void);
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
pub fn restore_foreground_hwnd(_hwnd_raw: isize) {}

/// Positions the launcher horizontally centered and near the upper third of the monitor
/// currently containing the mouse cursor.
#[cfg(windows)]
pub fn position_on_cursor_monitor(window: &WebviewWindow) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let Ok(win_size) = window.outer_size() else {
        let _ = window.center();
        return;
    };

    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            let _ = window.center();
            return;
        }
        let hmon = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmon, &mut info).as_bool() {
            let _ = window.center();
            return;
        }

        let work = info.rcWork;
        let mon_width = work.right - work.left;
        let mon_height = work.bottom - work.top;

        let x = work.left + (mon_width - win_size.width as i32) / 2;
        let y = work.top + (mon_height as f64 * 0.20) as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(not(windows))]
pub fn position_on_cursor_monitor(window: &WebviewWindow) {
    let _ = window.center();
}

/// Resizes the launcher window height in logical pixels while locking its top-left position
/// and skipping redundant OS resize calls to eliminate resize drift and flicker.
pub fn set_logical_height(window: &WebviewWindow, logical_width: f64, logical_height: f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let target_w = (logical_width * scale).round() as u32;
    let target_h = (logical_height * scale).round() as u32;

    if let Ok(current) = window.inner_size()
        && current.width == target_w
        && current.height == target_h
    {
        return;
    }

    let prev_pos = window.outer_position().ok();
    let _ = window.set_size(PhysicalSize::new(target_w, target_h));

    // Guard against Windows DPI/frame adjustments subtly drifting the window's top-left corner.
    if let (Some(before), Ok(after)) = (prev_pos, window.outer_position())
        && (before.x != after.x || before.y != after.y)
    {
        let _ = window.set_position(before);
    }
}
