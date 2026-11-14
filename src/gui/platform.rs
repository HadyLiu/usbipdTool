//! Windows 平台相关：系统深色模式检测、以管理员身份重新启动（清单失效时的兜底）。

/// 读取 Windows「应用模式」是否为深色（AppsUseLightTheme == 0）。
#[cfg(windows)]
pub fn system_dark() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let val: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 1;
    let mut size: u32 = 4;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            val.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    if rc == 0 { Some(data == 0) } else { None }
}

#[cfg(not(windows))]
pub fn system_dark() -> Option<bool> {
    None
}

/// 未获得管理员权限时，用 ShellExecuteW "runas" 重新启动自身。
/// 返回 true 表示已成功拉起提权进程（当前进程应退出）。
/// 正常情况下 EXE 内嵌 requireAdministrator 清单，不会走到这里。
#[cfg(windows)]
pub fn relaunch_as_admin() -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let Ok(exe) = std::env::current_exe() else { return false };
    let to_w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let exe_w = to_w(&exe.to_string_lossy());
    let verb = to_w("runas");
    let params = to_w("--elevated");
    let h = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe_w.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    (h as isize) > 32
}

#[cfg(not(windows))]
pub fn relaunch_as_admin() -> bool {
    false
}
