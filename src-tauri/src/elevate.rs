//! Running as administrator. Some build folders are created by an elevated terminal or
//! another account, and only an elevated DevSweep can delete them.

/// True when this process runs with an elevated (administrator) token.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Starts a second copy of DevSweep through the UAC prompt. The new copy waits for this one
/// to exit (they share the WebView2 profile) and, with `scan`, starts scanning right away.
/// Returns an error when the user declines the prompt.
#[cfg(windows)]
pub fn relaunch_elevated(scan: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut params = format!("--after {}", std::process::id());
    if scan {
        params.push_str(" --scan");
    }
    let verb = wide("runas".as_ref());
    let file = wide(exe.as_os_str());
    let params = wide(params.as_ref());

    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            let err = std::io::Error::last_os_error();
            return Err(if err.raw_os_error() == Some(1223) {
                "The administrator prompt was cancelled".into()
            } else {
                format!("Could not restart as administrator: {err}")
            });
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn relaunch_elevated(_scan: bool) -> Result<(), String> {
    Err("Restarting as administrator is only supported on Windows".into())
}

/// Called at startup with the `--after <pid>` argument: waits up to five seconds for the
/// copy that launched us to close, so two processes never open the WebView2 profile at once.
#[cfg(windows)]
pub fn wait_for_parent(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !handle.is_null() {
            WaitForSingleObject(handle, 5000);
            CloseHandle(handle);
        }
    }
}

#[cfg(not(windows))]
pub fn wait_for_parent(_pid: u32) {}
