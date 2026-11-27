use crate::search::index::{ItemKind, RawSearchItem};

/// Enumerates modern Windows UWP / MSIX applications from `shell:AppsFolder`.
#[cfg(windows)]
pub fn scan_uwp_apps() -> Vec<RawSearchItem> {
    use windows::core::{Interface, GUID};
    use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItem2,
        SHGetKnownFolderItem, KF_FLAG_DEFAULT, SIGDN_NORMALDISPLAY,
    };

    use crate::platform::scanner::win32::hash_icon_key;

    // PKEY_AppUserModel_ID = {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, pid 5
    const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

    let mut results = Vec::new();

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = hr.is_ok();

        let scan_result = (|| -> windows::core::Result<()> {
            let folder: IShellItem =
                SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)?;
            let enumerator: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)?;

            loop {
                let mut item_arr: [Option<IShellItem>; 1] = [None];
                let mut fetched = 0u32;
                let next_hr = enumerator.Next(&mut item_arr, Some(&mut fetched));
                if next_hr.is_err() || fetched == 0 {
                    break;
                }
                let Some(item) = item_arr[0].take() else {
                    break;
                };

                let Ok(item2) = item.cast::<IShellItem2>() else {
                    continue;
                };

                let Ok(aumid_pwstr) = item2.GetString(&PKEY_APP_USER_MODEL_ID) else {
                    continue;
                };
                let aumid = aumid_pwstr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(aumid_pwstr.0.cast()));

                // Filter to genuine UWP/MSIX packaged apps (AUMID contains `!`), avoiding
                // duplicating Win32 Start Menu apps that win32.rs already discovers.
                if aumid.is_empty() || !aumid.contains('!') {
                    continue;
                }

                let Ok(name_pwstr) = item.GetDisplayName(SIGDN_NORMALDISPLAY) else {
                    continue;
                };
                let name = name_pwstr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(name_pwstr.0.cast()));

                let trimmed_name = name.trim();
                if trimmed_name.is_empty() {
                    continue;
                }

                let icon_source = format!("shell:AppsFolder\\{aumid}");
                let icon_key = hash_icon_key(&icon_source);

                results.push(RawSearchItem {
                    id: format!("uwp:{aumid}"),
                    kind: ItemKind::UwpApp,
                    name: trimmed_name.to_string(),
                    subtitle: "Windows 应用".to_string(),
                    exec_path: aumid,
                    exec_args: String::new(),
                    working_dir: String::new(),
                    icon_key,
                    keywords: Vec::new(),
                });
            }

            Ok(())
        })();

        if let Err(err) = scan_result {
            tracing::warn!("Failed to enumerate UWP apps from shell:AppsFolder: {err}");
        }

        if initialized {
            CoUninitialize();
        }
    }

    results
}

#[cfg(not(windows))]
pub fn scan_uwp_apps() -> Vec<RawSearchItem> {
    Vec::new()
}
