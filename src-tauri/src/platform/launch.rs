use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};
use crate::search::index::{ItemKind, RawSearchItem};

/// Launches an indexed item (Win32 executable/shortcut, UWP AUMID, or built-in system command).
pub fn launch_search_item(item: &RawSearchItem, as_admin: bool) -> AppResult<()> {
    match item.kind {
        ItemKind::App => launch_win32(&item.exec_path, &item.exec_args, &item.working_dir, as_admin),
        ItemKind::UwpApp => launch_uwp(&item.exec_path),
        ItemKind::SystemCmd => execute_system_command(&item.exec_path),
        ItemKind::Plugin => Err(AppError::InvalidInput(
            "Plugin items must be opened via open_plugin".to_string(),
        )),
    }
}

/// Opens Windows File Explorer selecting the target executable or shortcut.
pub fn reveal_in_explorer(path: &str) -> AppResult<()> {
    let target = Path::new(path);
    if !target.exists() {
        return Err(AppError::NotFound(format!("Path does not exist: {path}")));
    }
    Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()?;
    Ok(())
}

/// Opens a directory directly inside Windows File Explorer (creating it if needed).
pub fn open_directory(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe").arg(path).spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(AppError::Platform(format!(
            "Opening directory '{}' is only implemented on Windows",
            path.display()
        )))
    }
}

#[cfg(windows)]
fn launch_win32(exec_path: &str, args: &str, working_dir: &str, as_admin: bool) -> AppResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb = if as_admin { "runas" } else { "open" };
    let verb_w = to_wide_z(verb);
    let file_w = to_wide_z(exec_path);
    let args_w = to_wide_z(args);
    let dir_w = to_wide_z(working_dir);

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(file_w.as_ptr()),
            if args.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(args_w.as_ptr())
            },
            if working_dir.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(dir_w.as_ptr())
            },
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns an HINSTANCE > 32 on success.
    if (result.0 as isize) <= 32 {
        return Err(AppError::Platform(format!(
            "ShellExecuteW failed for '{exec_path}' (code {})",
            result.0 as isize
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_win32(exec_path: &str, _args: &str, _working_dir: &str, _as_admin: bool) -> AppResult<()> {
    Err(AppError::Platform(format!(
        "Launching '{exec_path}' is only supported on Windows"
    )))
}

#[cfg(windows)]
fn launch_uwp(aumid: &str) -> AppResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        ApplicationActivationManager, IApplicationActivationManager, AO_NONE,
    };

    let aumid_w = to_wide_z(aumid);

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = hr.is_ok();

        let activation = (|| -> windows::core::Result<()> {
            let mgr: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)?;
            let _pid = mgr.ActivateApplication(PCWSTR(aumid_w.as_ptr()), PCWSTR::null(), AO_NONE)?;
            Ok(())
        })();

        if initialized {
            CoUninitialize();
        }

        activation.map_err(|e| AppError::Platform(format!("Failed to launch UWP '{aumid}': {e}")))
    }
}

#[cfg(not(windows))]
fn launch_uwp(aumid: &str) -> AppResult<()> {
    Err(AppError::Platform(format!(
        "Launching UWP '{aumid}' is only supported on Windows"
    )))
}

fn execute_system_command(cmd_uri: &str) -> AppResult<()> {
    match cmd_uri {
        "sys:lock" => lock_workstation(),
        "sys:sleep" => sleep_workstation(),
        "sys:restart" => {
            Command::new("shutdown").args(["/r", "/t", "0"]).spawn()?;
            Ok(())
        }
        "sys:shutdown" => {
            Command::new("shutdown").args(["/s", "/t", "0"]).spawn()?;
            Ok(())
        }
        "sys:taskmgr" => launch_win32("taskmgr.exe", "", "", false),
        "sys:control" => launch_win32("control.exe", "", "", false),
        "sys:env" => launch_win32("rundll32.exe", "sysdm.cpl,EditEnvironmentVariables", "", false),
        "sys:devmgmt" => launch_win32("devmgmt.msc", "", "", false),
        "sys:settings" => launch_win32("ms-settings:", "", "", false),
        "sys:explorer" => launch_win32("explorer.exe", "", "", false),
        "sys:recycle-bin" => launch_win32("explorer.exe", "shell:RecycleBinFolder", "", false),
        other => Err(AppError::NotFound(format!("Unknown system command: {other}"))),
    }
}

#[cfg(windows)]
fn lock_workstation() -> AppResult<()> {
    use windows::Win32::System::Shutdown::LockWorkStation;
    unsafe {
        LockWorkStation().map_err(|e| AppError::Platform(format!("LockWorkStation failed: {e}")))
    }
}

#[cfg(not(windows))]
fn lock_workstation() -> AppResult<()> {
    Ok(())
}

#[cfg(windows)]
fn sleep_workstation() -> AppResult<()> {
    use windows::Win32::System::Power::SetSuspendState;
    let ok = unsafe { SetSuspendState(false, false, false) };
    if ok.as_bool() {
        Ok(())
    } else {
        Err(AppError::Platform("SetSuspendState failed".to_string()))
    }
}

#[cfg(not(windows))]
fn sleep_workstation() -> AppResult<()> {
    Ok(())
}

#[cfg(windows)]
fn to_wide_z(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
