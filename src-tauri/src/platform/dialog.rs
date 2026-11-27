use crate::error::{AppError, AppResult};

/// Opens a native OS file picker filtered for `.txz` and `.zip` Triex plugin packages.
#[cfg(windows)]
pub fn open_plugin_file_dialog(owner_hwnd: isize) -> AppResult<Option<String>> {
    use windows::core::w;
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FileOpenDialog,
        IFileOpenDialog, SIGDN_FILESYSPATH,
    };

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = hr.is_ok();

        let dialog_res = (|| -> windows::core::Result<Option<String>> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetTitle(w!("选择 Triex 插件安装包"))?;
            dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM)?;

            let filters = [
                COMDLG_FILTERSPEC {
                    pszName: w!("Triex 插件安装包 (*.txz;*.zip)"),
                    pszSpec: w!("*.txz;*.zip"),
                },
                COMDLG_FILTERSPEC {
                    pszName: w!("所有文件 (*.*)"),
                    pszSpec: w!("*.*"),
                },
            ];
            dialog.SetFileTypes(&filters)?;
            dialog.SetFileTypeIndex(1)?;

            let owner = HWND(owner_hwnd as *mut std::ffi::c_void);
            if let Err(err) = dialog.Show(owner) {
                if err.code() == ERROR_CANCELLED.to_hresult() {
                    return Ok(None);
                }
                return Err(err);
            }

            let item = dialog.GetResult()?;
            let path_pwstr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = path_pwstr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(path_pwstr.0.cast()));

            if path_str.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path_str))
            }
        })();

        if initialized {
            CoUninitialize();
        }

        dialog_res.map_err(|err| AppError::Platform(format!("Failed to open file dialog: {err}")))
    }
}

#[cfg(not(windows))]
pub fn open_plugin_file_dialog(_owner_hwnd: isize) -> AppResult<Option<String>> {
    Err(AppError::Platform(
        "Native file dialog is only implemented for Windows".into(),
    ))
}
