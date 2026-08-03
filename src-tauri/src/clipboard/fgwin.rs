//! Foreground window title + module name (Windows only).
//!
//! Cached briefly so bursty clipboard events don't OpenProcess every time.

/// Capture the foreground window's title and module name (Windows only).
/// Cached briefly so bursty clipboard events don't OpenProcess every time.
#[cfg(windows)]
pub fn get_foreground_window_info() -> (String, String) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static CACHE: Mutex<Option<(Instant, String, String)>> = Mutex::new(None);
    const TTL: Duration = Duration::from_millis(250);

    if let Ok(guard) = CACHE.lock() {
        if let Some((at, title, app)) = guard.as_ref() {
            if at.elapsed() < TTL {
                return (title.clone(), app.clone());
            }
        }
    }

    let info = get_foreground_window_info_uncached();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), info.0.clone(), info.1.clone()));
    }
    info
}

#[cfg(windows)]
fn get_foreground_window_info_uncached() -> (String, String) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows_sys::Win32::Foundation::CloseHandle;
    // PROCESS_QUERY_LIMITED_INFORMATION + QueryFullProcessImageNameW works across
    // integrity levels without PROCESS_VM_READ. GetModuleFileNameW is wrong here —
    // it expects an HMODULE in *this* process, not a foreign process HANDLE.
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (String::new(), String::new());
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = if len > 0 {
            OsString::from_wide(&title_buf[..len as usize])
                .to_string_lossy()
                .to_string()
        } else {
            String::new()
        };

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return (title, String::new());
        }

        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process_handle.is_null() {
            return (title, String::new());
        }

        let mut module_buf = [0u16; 260];
        let mut size = module_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process_handle,
            0,
            module_buf.as_mut_ptr(),
            &mut size,
        );
        CloseHandle(process_handle);

        let module = if ok != 0 && size > 0 {
            let path = OsString::from_wide(&module_buf[..size as usize])
                .to_string_lossy()
                .to_string();
            std::path::Path::new(&path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(path)
        } else {
            String::new()
        };

        (title, module)
    }
}

#[cfg(not(windows))]
pub fn get_foreground_window_info() -> (String, String) {
    (String::new(), String::new())
}
