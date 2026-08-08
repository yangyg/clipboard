//! Paste-target HWND tracking and focus / Ctrl+V simulation (Windows).
//!
//! The monitor keeps the last non-Clipboard foreground window fresh; paste
//! then focuses it (holding foreground rights) before simulating Ctrl+V.

use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

/// Paste target HWND: last non-Clipboard foreground window (kept fresh by tracker).
static PASTE_TARGET_HWND: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);
/// Our main window HWND (tao/WebView2 top-level). Needed because FG may be owned by
/// the WebView2 process (different PID) yet still be our UI.
static OUR_MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

/// Simulate Ctrl+V after clipboard content has been set.
/// Caller should delay (~80ms) after focusing the target so the window is ready.
#[cfg(windows)]
pub fn simulate_paste_keys() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    unsafe {
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
pub fn simulate_paste_keys() {
    warn!("Paste simulation not available on this platform");
}

#[cfg(windows)]
fn root_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOT};
    unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        if root.is_null() {
            hwnd
        } else {
            root
        }
    }
}

/// Cache our main HWND so the background tracker can ignore WebView2-hosted FG.
pub fn set_our_main_hwnd(hwnd: Option<isize>) {
    OUR_MAIN_HWND.store(hwnd.unwrap_or(0), Ordering::SeqCst);
}

fn our_main_hwnd() -> Option<isize> {
    let v = OUR_MAIN_HWND.load(Ordering::SeqCst);
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

/// True if `hwnd` is our panel (same process, same root, or WebView2 child of our window).
#[cfg(windows)]
pub fn hwnd_belongs_to_us(
    hwnd: windows_sys::Win32::Foundation::HWND,
    our_hwnd: Option<isize>,
) -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetParent, GetWindowThreadProcessId};

    if hwnd.is_null() {
        return false;
    }
    let our = our_hwnd.or_else(our_main_hwnd);
    unsafe {
        if let Some(our) = our {
            let our_h = our as windows_sys::Win32::Foundation::HWND;
            if !our_h.is_null() {
                if hwnd == our_h || root_hwnd(hwnd) == root_hwnd(our_h) {
                    return true;
                }
                // Walk parents — WebView2 host HWNDs sit under our top-level frame.
                let mut cur = hwnd;
                for _ in 0..24 {
                    if cur.is_null() {
                        break;
                    }
                    if cur == our_h || root_hwnd(cur) == root_hwnd(our_h) {
                        return true;
                    }
                    let parent = GetParent(cur);
                    if parent.is_null() || parent == cur {
                        break;
                    }
                    cur = parent;
                }
            }
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid != 0 && pid == GetCurrentProcessId()
    }
}

/// Continuously record the last foreign foreground window.
#[cfg(windows)]
pub fn track_last_foreign_foreground() {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    // H-1: Cache last-seen foreground HWND. The expensive hwnd_belongs_to_us
    // parent-walk (up to 24 levels) only runs when the user actually switches
    // windows, not on every 250ms poll tick.
    static LAST_SEEN_FG: AtomicIsize = AtomicIsize::new(0);

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return;
        }
        let fg_val = fg as isize;
        // Fast path: same window as last tick → skip all further work.
        if LAST_SEEN_FG.load(Ordering::Relaxed) == fg_val {
            return;
        }
        LAST_SEEN_FG.store(fg_val, Ordering::Relaxed);

        if hwnd_belongs_to_us(fg, our_main_hwnd()) {
            return;
        }
        let target = root_hwnd(fg) as isize;
        let mut slot = PASTE_TARGET_HWND.lock();
        if *slot != Some(target) {
            *slot = Some(target);
            debug!("Tracked paste target hwnd={:#x}", target);
        }
    }
}

#[cfg(not(windows))]
pub fn track_last_foreign_foreground() {}

/// Snapshot the current foreground window as the paste destination, unless it is us.
#[cfg(windows)]
pub fn remember_paste_target(our_hwnd: Option<isize>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    if our_hwnd.is_some() {
        set_our_main_hwnd(our_hwnd);
    }

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return;
        }
        if hwnd_belongs_to_us(fg, our_hwnd) {
            return;
        }
        let target = root_hwnd(fg) as isize;
        *PASTE_TARGET_HWND.lock() = Some(target);
        debug!("Remembered paste target hwnd={:#x}", target);
    }
}

#[cfg(not(windows))]
pub fn remember_paste_target(_our_hwnd: Option<isize>) {}

/// After we hide, Windows may already restore the previous app — enough for Ctrl+V.
#[cfg(windows)]
pub fn foreground_is_pasteable(our_hwnd: Option<isize>) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let fg = GetForegroundWindow();
        !fg.is_null() && !hwnd_belongs_to_us(fg, our_hwnd.or_else(our_main_hwnd))
    }
}

#[cfg(not(windows))]
pub fn foreground_is_pasteable(_our_hwnd: Option<isize>) -> bool {
    false
}

/// Bring a window to the foreground so simulated Ctrl+V lands in it.
/// Call while we still hold foreground rights (before hide) when possible.
#[cfg(windows)]
pub fn focus_window(hwnd_id: isize) -> bool {
    use windows_sys::Win32::Foundation::{FALSE, TRUE};
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
        IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SwitchToThisWindow,
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETFOREGROUNDLOCKTIMEOUT,
        SPI_SETFOREGROUNDLOCKTIMEOUT, SW_RESTORE,
    };

    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return false;
        }
        let hwnd = root_hwnd(hwnd);

        let fg_now = GetForegroundWindow();
        if !fg_now.is_null() && root_hwnd(fg_now) == hwnd {
            return true;
        }

        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        // Bypass foreground lock timeout (clipboard-manager standard technique).
        let mut lock_timeout: u32 = 0;
        let got_timeout = SystemParametersInfoW(
            SPI_GETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut lock_timeout as *mut u32 as *mut _,
            0,
        ) != 0;
        let mut zero: u32 = 0;
        let _ = SystemParametersInfoW(
            SPI_SETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut zero as *mut u32 as *mut _,
            SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
        );

        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        let mut target_pid = 0u32;
        let fg_tid = if !fg.is_null() {
            GetWindowThreadProcessId(fg, &mut fg_pid)
        } else {
            0
        };
        let target_tid = GetWindowThreadProcessId(hwnd, &mut target_pid);
        let cur_tid = GetCurrentThreadId();

        keybd_event(VK_MENU as u8, 0, 0, 0);
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);

        if target_pid != 0 {
            AllowSetForegroundWindow(target_pid);
        }
        AllowSetForegroundWindow(u32::MAX);

        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, TRUE);
        }
        if target_tid != 0 && target_tid != cur_tid && target_tid != fg_tid {
            AttachThreadInput(cur_tid, target_tid, TRUE);
        }

        BringWindowToTop(hwnd);
        SwitchToThisWindow(hwnd, TRUE);
        let _ = SetForegroundWindow(hwnd);

        if target_tid != 0 && target_tid != cur_tid && target_tid != fg_tid {
            AttachThreadInput(cur_tid, target_tid, FALSE);
        }
        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, FALSE);
        }

        // Only restore when the read succeeded. Restoring a zero value would
        // persist "foreground lock disabled" system-wide (SPIF_UPDATEINIFILE).
        if got_timeout {
            let _ = SystemParametersInfoW(
                SPI_SETFOREGROUNDLOCKTIMEOUT,
                0,
                &mut lock_timeout as *mut u32 as *mut _,
                SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
            );
        }

        let now = GetForegroundWindow();
        let ok = !now.is_null() && root_hwnd(now) == hwnd;
        if ok {
            debug!("Focused paste target hwnd={:#x}", hwnd as isize);
        } else {
            warn!(
                "SetForegroundWindow failed for hwnd={:#x} (fg={:#x})",
                hwnd as isize, now as isize
            );
        }
        ok
    }
}

#[cfg(not(windows))]
pub fn focus_window(_hwnd_id: isize) -> bool {
    false
}

/// Whether `hwnd_id` (or its root owner) is the current foreground window.
#[cfg(windows)]
pub fn is_foreground_hwnd(hwnd_id: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};
    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return false;
        }
        let fg = GetForegroundWindow();
        !fg.is_null() && root_hwnd(fg) == root_hwnd(hwnd)
    }
}

#[cfg(not(windows))]
pub fn is_foreground_hwnd(_hwnd_id: isize) -> bool {
    false
}

/// Hide via Win32 so focus can leave us even if Tauri hide is delayed.
#[cfg(windows)]
pub fn hide_hwnd(hwnd_id: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub fn hide_hwnd(_hwnd_id: isize) {}

/// Prefer the tracked foreign window. Fall back to current FG if foreign.
#[cfg(windows)]
pub fn resolve_paste_target(our_hwnd: Option<isize>) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};

    if our_hwnd.is_some() {
        set_our_main_hwnd(our_hwnd);
    }
    track_last_foreign_foreground();

    let our = our_hwnd.or_else(our_main_hwnd);
    let remembered = *PASTE_TARGET_HWND.lock();
    if let Some(id) = remembered {
        let hwnd = id as windows_sys::Win32::Foundation::HWND;
        unsafe {
            if !hwnd.is_null() && IsWindow(hwnd) != 0 && !hwnd_belongs_to_us(hwnd, our) {
                return Some(root_hwnd(hwnd) as isize);
            }
        }
    }

    unsafe {
        let fg = GetForegroundWindow();
        if !fg.is_null() && !hwnd_belongs_to_us(fg, our) {
            let id = root_hwnd(fg) as isize;
            *PASTE_TARGET_HWND.lock() = Some(id);
            return Some(id);
        }
    }
    None
}

#[cfg(not(windows))]
pub fn resolve_paste_target(_our_hwnd: Option<isize>) -> Option<isize> {
    None
}
