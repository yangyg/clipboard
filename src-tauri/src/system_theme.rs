//! Native OS light/dark mode change detection (Windows).
//!
//! WebView2 does not reliably fire `prefers-color-scheme` change events while
//! its host window is hidden — and the floating panel is hidden most of the
//! time. So "follow system" cannot rely on `matchMedia` alone. Instead we
//! watch the OS natively: Windows broadcasts `WM_SETTINGCHANGE` with lParam
//! "ImmersiveColorSet" whenever the apps light/dark mode is toggled, and the
//! authoritative value lives in the registry at
//! HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme
//! (0 = dark, 1 = light; also honoured in "custom" mode).
//!
//! On change we emit `system-theme-changed` (payload: `dark: bool`) to every
//! webview; frontends apply it only when the user chose the "system" theme.

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicIsize, Ordering};
#[cfg(windows)]
use std::sync::OnceLock;

/// -1 = unknown, 0 = light, 1 = dark — used to dedupe repeated broadcasts.
#[cfg(windows)]
static LAST_DARK: AtomicI8 = AtomicI8::new(-1);
#[cfg(windows)]
static WATCHER_HWND: AtomicIsize = AtomicIsize::new(0);
/// Double-start guard (same pattern as tray::RESUME_WATCHER_STARTED).
#[cfg(windows)]
static WATCHER_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

#[cfg(windows)]
pub(crate) fn start_system_theme_watcher(app: tauri::AppHandle) {
    if WATCHER_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let _ = APP_HANDLE.set(app);
    let _ = std::thread::Builder::new()
        .name("system-theme-watcher".into())
        .spawn(watcher_thread);
}

#[cfg(not(windows))]
pub(crate) fn start_system_theme_watcher(_app: tauri::AppHandle) {}

/// Politely end the watcher's message loop on app exit.
pub(crate) fn stop_system_theme_watcher() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
        let hwnd = WATCHER_HWND.load(Ordering::Relaxed);
        if hwnd != 0 {
            unsafe { PostMessageW(hwnd as _, WM_CLOSE, 0, 0) };
        }
    }
}

#[cfg(windows)]
fn watcher_thread() {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage, MSG,
        WNDCLASSW,
    };

    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name: Vec<u16> = "ClipVaultThemeWatcher\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(watcher_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            tracing::warn!("system theme watcher: RegisterClassW failed");
            return;
        }

        // Invisible *top-level* window: message-only windows (HWND_MESSAGE)
        // do not receive broadcast WM_SETTINGCHANGE, so we must not use one.
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0, // no WS_VISIBLE — window is never shown
            0,
            0,
            0,
            0,
            std::ptr::null_mut(), // no parent — top-level so broadcasts arrive
            std::ptr::null_mut(), // no menu
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            tracing::warn!("system theme watcher: CreateWindowExW failed");
            return;
        }
        WATCHER_HWND.store(hwnd as isize, Ordering::Relaxed);

        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn watcher_wndproc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, PostQuitMessage, WM_CLOSE, WM_DESTROY, WM_SETTINGCHANGE,
    };

    match msg {
        WM_SETTINGCHANGE => {
            if is_immersive_color_set(lparam) {
                if let Some(dark) = read_apps_use_dark_mode() {
                    let encoded = i8::from(dark);
                    if LAST_DARK.swap(encoded, Ordering::Relaxed) != encoded {
                        if let Some(app) = APP_HANDLE.get() {
                            use tauri::Emitter;
                            let _ = app.emit("system-theme-changed", dark);
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// True when the broadcast carries the "ImmersiveColorSet" marker (the
/// signal Windows sends after the apps light/dark mode changes).
#[cfg(windows)]
fn is_immersive_color_set(lparam: windows_sys::Win32::Foundation::LPARAM) -> bool {
    if lparam == 0 {
        return false;
    }
    let ptr = lparam as *const u16;
    let mut len = 0usize;
    unsafe {
        while len < 256 && *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)) == "ImmersiveColorSet"
    }
}

/// Current apps-mode from the registry: `Some(true)` = dark.
#[cfg(windows)]
fn read_apps_use_dark_mode() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_DWORD,
    };

    unsafe {
        let subkey: Vec<u16> =
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
                .encode_utf16()
                .collect();
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
        let mut data: u32 = 1;
        let mut size: u32 = 4;
        let mut kind: u32 = 0;
        let status = RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            &mut data as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(key);
        if status != 0 || kind != REG_DWORD {
            return None;
        }
        Some(data == 0)
    }
}
