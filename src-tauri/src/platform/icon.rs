use std::path::Path;

use crate::error::AppResult;

/// Extracts a 64x64 RGBA icon from any Win32 executable/shortcut or `shell:AppsFolder\<AUMID>`
/// via `IShellItemImageFactory` and persists it as a PNG file.
#[cfg(windows)]
pub fn ensure_icon_cached(shell_parsing_name: &str, dest_png: &Path) -> AppResult<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    use image::{ImageBuffer, Rgba};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
        SIIGBF_RESIZETOFIT,
    };

    if dest_png.exists() {
        return Ok(());
    }

    let wide: Vec<u16> = std::ffi::OsStr::new(shell_parsing_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = hr.is_ok();

        let extract_result = (|| -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;

            let size = SIZE { cx: 64, cy: 64 };
            // Try icon-only first for crisp app icons, then fall back to general shell thumbnail.
            let hbitmap: HBITMAP = factory
                .GetImage(size, SIIGBF_ICONONLY | SIIGBF_RESIZETOFIT)
                .or_else(|_| factory.GetImage(size, SIIGBF_RESIZETOFIT))
                .ok()?;

            let mut bm = BITMAP::default();
            let got = GetObjectW(
                hbitmap,
                std::mem::size_of::<BITMAP>() as i32,
                Some((&mut bm as *mut BITMAP).cast::<c_void>()),
            );
            if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
                let _ = DeleteObject(hbitmap);
                return None;
            }

            let width = bm.bmWidth as u32;
            let height = bm.bmHeight as u32;

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bm.bmWidth,
                    // Negative height requests top-down DIB ordering.
                    biHeight: -bm.bmHeight,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bgra = vec![0u8; (width * height * 4) as usize];
            let hdc = CreateCompatibleDC(None);
            let lines = GetDIBits(
                hdc,
                hbitmap,
                0,
                height,
                Some(bgra.as_mut_ptr().cast::<c_void>()),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(hdc);
            let _ = DeleteObject(hbitmap);

            if lines == 0 {
                return None;
            }

            // Convert BGRA -> RGBA in place.
            for px in bgra.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }

            ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, bgra)
        })();

        if initialized {
            CoUninitialize();
        }

        if let Some(img) = extract_result {
            if let Some(parent) = dest_png.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = img.save(dest_png);
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn ensure_icon_cached(_shell_parsing_name: &str, _dest_png: &Path) -> AppResult<()> {
    Ok(())
}
