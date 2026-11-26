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
