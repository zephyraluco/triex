use serde::{Deserialize, Serialize};

/// Summary of the current clipboard content for smart action / plugin matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSnapshot {
    pub kind: String,
    pub preview: String,
}

/// Reads plain UTF-16 text from the Windows clipboard if available.
#[cfg(windows)]
pub fn read_clipboard_text() -> Option<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;

    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }

        let text = (|| -> Option<String> {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 && len < 65_536 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let s = String::from_utf16_lossy(slice);
            let _ = GlobalUnlock(hglobal);
            Some(s)
        })();

        let _ = CloseClipboard();
        text
    }
}

#[cfg(not(windows))]
pub fn read_clipboard_text() -> Option<String> {
    None
}

/// Writes plain UTF-16 text into the Windows system clipboard.
#[cfg(windows)]
pub fn write_clipboard_text(text: &str) -> crate::error::AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;

    let wide: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        OpenClipboard(None)
            .map_err(|e| crate::error::AppError::Platform(format!("OpenClipboard failed: {e}")))?;

        let write_res = (|| -> crate::error::AppResult<()> {
            EmptyClipboard().map_err(|e| {
                crate::error::AppError::Platform(format!("EmptyClipboard failed: {e}"))
            })?;
            let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len).map_err(|e| {
                crate::error::AppError::Platform(format!("GlobalAlloc failed: {e}"))
            })?;
            let ptr = GlobalLock(hmem) as *mut u16;
            if ptr.is_null() {
                let _ = GlobalFree(hmem);
                return Err(crate::error::AppError::Platform(
                    "GlobalLock returned null".to_string(),
                ));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(hmem);

            if SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0)).is_err() {
                let _ = GlobalFree(hmem);
                return Err(crate::error::AppError::Platform(
                    "SetClipboardData failed".to_string(),
                ));
            }
            Ok(())
        })();

        let _ = CloseClipboard();
        write_res
    }
}

#[cfg(not(windows))]
pub fn write_clipboard_text(_text: &str) -> crate::error::AppResult<()> {
    Ok(())
}

pub fn current_clipboard_snapshot() -> Option<ClipboardSnapshot> {
    let raw = read_clipboard_text()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let kind = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        "url"
    } else if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        "json"
    } else {
        "text"
    };

    let preview: String = trimmed.chars().take(120).collect();
    Some(ClipboardSnapshot {
        kind: kind.to_string(),
        preview,
    })
}
