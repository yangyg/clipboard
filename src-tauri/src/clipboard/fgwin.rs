//! Foreground window title + module name (Windows only).
//!
//! Cached briefly so bursty clipboard events don't OpenProcess every time.

/// Capture the foreground window's title, module name and friendly display name
/// (from the exe's version resource `FileDescription`). Windows only.
/// Cached briefly so bursty clipboard events don't OpenProcess every time.
#[cfg(windows)]
pub fn get_foreground_window_info() -> (String, String, String) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static CACHE: Mutex<Option<(Instant, String, String, String)>> = Mutex::new(None);
    const TTL: Duration = Duration::from_millis(250);

    if let Ok(guard) = CACHE.lock() {
        if let Some((at, title, app, friendly)) = guard.as_ref() {
            if at.elapsed() < TTL {
                return (title.clone(), app.clone(), friendly.clone());
            }
        }
    }

    let info = get_foreground_window_info_uncached();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((
            Instant::now(),
            info.0.clone(),
            info.1.clone(),
            info.2.clone(),
        ));
    }
    info
}

#[cfg(windows)]
fn get_foreground_window_info_uncached() -> (String, String, String) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };
    // PROCESS_QUERY_LIMITED_INFORMATION + QueryFullProcessImageNameW works across
    // integrity levels without PROCESS_VM_READ. GetModuleFileNameW is wrong here —
    // it expects an HMODULE in *this* process, not a foreign process HANDLE.
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (String::new(), String::new(), String::new());
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
            return (title, String::new(), String::new());
        }

        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process_handle.is_null() {
            return (title, String::new(), String::new());
        }

        let mut module_buf = [0u16; 260];
        let mut size = module_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process_handle, 0, module_buf.as_mut_ptr(), &mut size);
        CloseHandle(process_handle);

        if ok == 0 || size == 0 {
            return (title, String::new(), String::new());
        }

        let path = OsString::from_wide(&module_buf[..size as usize])
            .to_string_lossy()
            .to_string();
        let module = std::path::Path::new(&path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(path.clone());

        // FileDescription from the version resource (Chinese apps usually ship a
        // Chinese name here). Cached per-path so app switches don't re-read files.
        let mut friendly = friendly_name_for_path(&path);
        if friendly.eq_ignore_ascii_case(&module) {
            friendly = String::new();
        }

        (title, module, friendly)
    }
}

/// Read the `FileDescription` string from an exe's version resource.
/// Returns `None` when the file has no version info / no readable description.
#[cfg(windows)]
fn read_file_description(path: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut handle: u32 = 0;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }

        let mut buf: Vec<u8> = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), handle, size, buf.as_mut_ptr() as *mut _) == 0 {
            return None;
        }

        // Enumerate translations: the FileDescription sub-block path is
        // language/codepage-specific, so hard-coding `040904B0` misses Chinese
        // version resources. Each entry is a u32: (lang | codepage << 16).
        let translation_key: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
        let mut trans_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut trans_len: u32 = 0;
        if VerQueryValueW(
            buf.as_ptr() as *const _,
            translation_key.as_ptr(),
            &mut trans_ptr,
            &mut trans_len,
        ) == 0
            || trans_ptr.is_null()
            || trans_len < 4
        {
            return None;
        }
        let first = *(trans_ptr as *const u32);

        let sub_key = format!(
            "\\StringFileInfo\\{:04X}{:04X}\\FileDescription\0",
            first & 0xFFFF,
            (first >> 16) & 0xFFFF,
        );
        let sub_key_wide: Vec<u16> = sub_key.encode_utf16().collect();

        let mut desc_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut desc_len: u32 = 0;
        if VerQueryValueW(
            buf.as_ptr() as *const _,
            sub_key_wide.as_ptr(),
            &mut desc_ptr,
            &mut desc_len,
        ) == 0
            || desc_ptr.is_null()
            || desc_len == 0
        {
            return None;
        }

        // VerQueryValueW reports string lengths in UTF-16 characters, not
        // bytes. Dividing this value by two truncates every description.
        let wide_desc = std::slice::from_raw_parts(desc_ptr as *const u16, desc_len as usize);
        decode_file_description(wide_desc)
    }
}

#[cfg(windows)]
fn decode_file_description(wide_desc: &[u16]) -> Option<String> {
    let s = String::from_utf16_lossy(wide_desc)
        .trim_matches('\0')
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Cache friendly names per full exe path (bounded) so switching between the
/// same apps doesn't re-read the version resource on every TTL expiry.
#[cfg(windows)]
fn friendly_name_for_path(path: &str) -> String {
    use std::collections::HashMap;
    use std::sync::Mutex;

    static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    const MAX_ENTRIES: usize = 512;

    match CACHE.lock() {
        Ok(mut guard) => {
            let map = guard.get_or_insert_with(HashMap::new);
            if let Some(name) = map.get(path) {
                return name.clone();
            }
            let name = read_file_description(path).unwrap_or_default();
            if map.len() >= MAX_ENTRIES {
                map.clear();
            }
            map.insert(path.to_string(), name.clone());
            name
        }
        Err(_) => read_file_description(path).unwrap_or_default(),
    }
}

#[cfg(not(windows))]
pub fn get_foreground_window_info() -> (String, String, String) {
    (String::new(), String::new(), String::new())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn file_description_keeps_the_full_utf16_string() {
        let description: Vec<u16> = "OpenCode 中文".encode_utf16().collect();

        assert_eq!(
            super::decode_file_description(&description),
            Some("OpenCode 中文".to_string())
        );
    }
}
