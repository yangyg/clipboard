//! System tray icon and event wiring. Right-click shows the custom
//! `tray-menu` window anchored to the tray icon; left-click toggles the main panel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position, Rect};
use tracing::{info, warn};

/// Logical size of the tray-menu window (must match `tauri.conf.json`).
const MENU_LOGICAL_W: f64 = 176.0;
/// Initial / fallback height; frontend resizes to content on open.
const MENU_LOGICAL_H: f64 = 148.0;
const MENU_GAP: f64 = 4.0;

/// Debounce resume recovery (multiple power events fire on wake).
static LAST_RESUME_MS: AtomicU64 = AtomicU64::new(0);
static RESUME_WATCHER_STARTED: AtomicBool = AtomicBool::new(false);
static POWER_APP: OnceLock<AppHandle> = OnceLock::new();

/// Clamp menu top-left so the menu stays inside the work area (physical px).
pub(crate) fn clamp_menu_position(
    preferred: (f64, f64),
    menu_size: (f64, f64),
    work: (f64, f64, f64, f64), // x, y, w, h
) -> (f64, f64) {
    let (cx, cy) = preferred;
    let (mw, mh) = menu_size;
    let (wx, wy, ww, wh) = work;
    let pad = 8.0;
    let max_x = wx + ww - mw - pad;
    let max_y = wy + wh - mh - pad;
    let x = cx.min(max_x).max(wx + pad);
    let y = cy.min(max_y).max(wy + pad);
    (x, y)
}

/// Prefer above the tray icon, right-aligned to the icon (Windows-like).
/// Falls back below / left-align when there is not enough room, then clamps.
pub(crate) fn anchor_menu_to_tray_icon(
    icon: (f64, f64, f64, f64), // x, y, w, h physical
    menu_size: (f64, f64),
    work: (f64, f64, f64, f64),
) -> (f64, f64) {
    let (ix, iy, iw, ih) = icon;
    let (mw, mh) = menu_size;
    let (wx, wy, _ww, _wh) = work;

    // Right-align to icon; prefer opening upward into the work area.
    let mut x = ix + iw - mw;
    let mut y = iy - mh - MENU_GAP;

    if y < wy {
        y = iy + ih + MENU_GAP;
    }
    if x < wx {
        x = ix;
    }

    clamp_menu_position((x, y), menu_size, work)
}

/// Build the system tray icon (no native menu) and register click handlers.
pub(crate) fn build_tray(
    app: &AppHandle,
    capture_paused: Arc<RwLock<bool>>,
) -> tauri::Result<()> {
    let _capture_paused = capture_paused;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("剪贴板管理")
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    crate::toggle_main_panel(app);
                }
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    position,
                    rect,
                    ..
                } => {
                    show_tray_menu(app, position, rect);
                }
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

/// After sleep/hibernate, WebView2 / tray registration can go stale.
/// Reload menu webview and rebuild the tray icon so clicks work again.
pub(crate) fn recover_after_resume(app: &AppHandle) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let prev = LAST_RESUME_MS.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < 2_000 {
        return;
    }
    LAST_RESUME_MS.store(now, Ordering::Relaxed);

    info!("System resume detected — recovering tray + tray-menu webview");

    // Give display / Explorer a moment to settle after wake.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        let app2 = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            recover_after_resume_inner(&app2);
        }) {
            warn!("Failed to schedule resume recovery on main thread: {e}");
        }
    });
}

fn recover_after_resume_inner(app: &AppHandle) {
    // Reload tray-menu (and main) — WebView2 content process may be dead after sleep.
    for label in ["tray-menu", "main"] {
        if let Some(w) = app.get_webview_window(label) {
            if let Err(e) = w.eval("try{location.reload()}catch(e){}") {
                warn!("Failed to reload webview '{label}' after resume: {e}");
            }
        }
    }

    // Rebuild tray so click handlers are re-registered if Explorer/tray went stale.
    let paused = app
        .try_state::<crate::AppState>()
        .map(|s| s.capture_paused.clone())
        .unwrap_or_else(|| Arc::new(RwLock::new(false)));

    let _ = app.remove_tray_by_id("main-tray");
    if let Err(e) = build_tray(app, paused) {
        warn!("Failed to rebuild tray after resume: {e}");
    } else {
        info!("Tray rebuilt after resume");
    }
}

/// Start a Windows message-only window that listens for power resume.
/// `RunEvent::Resumed` is unreliable for sleep/wake on Windows.
pub(crate) fn start_resume_watcher(app: AppHandle) {
    #[cfg(windows)]
    {
        if RESUME_WATCHER_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let _ = POWER_APP.set(app.clone());
        std::thread::Builder::new()
            .name("clipvault-power-watch".into())
            .spawn(windows_power_watch_loop)
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = &RESUME_WATCHER_STARTED;
        let _ = &POWER_APP;
    }
}

#[cfg(windows)]
fn windows_power_watch_loop() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, CW_USEDEFAULT, MSG, WM_DESTROY, WM_POWERBROADCAST, WNDCLASSW,
    };

    const PBT_APMRESUMESUSPEND: usize = 0x0007;
    const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
    const PBT_APMRESUMECRITICAL: usize = 0x0006;

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_POWERBROADCAST {
            let event = wparam as usize;
            if matches!(
                event,
                PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMECRITICAL
            ) {
                if let Some(app) = POWER_APP.get() {
                    recover_after_resume(app);
                }
            }
            return 1; // TRUE — broadcast handled
        }
        if msg == WM_DESTROY {
            return 0;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    unsafe {
        let class_name: Vec<u16> = "ClipVaultPowerWatch\0".encode_utf16().collect();
        let hinstance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
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
            warn!("Failed to register power-watch window class");
            return;
        }

        // Message-only window (HWND_MESSAGE parent).
        const HWND_MESSAGE: HWND = -3isize as HWND;
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            warn!("Failed to create power-watch message window");
            return;
        }

        info!("Power resume watcher started");
        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Subclass the tray-menu window so CSS `cursor: pointer` works.
///
/// WebView2 calls `SetCursor` internally when the pointer moves over
/// interactive elements, but the top-level `WM_SETCURSOR` handling
/// (`DefWindowProc`) immediately resets it to the class arrow — most
/// visible on transparent, decoration-less popup windows. Returning
/// TRUE for client-area cursor requests lets WebView2's own `SetCursor`
/// persist.
#[cfg(windows)]
pub(crate) fn hook_tray_menu_cursor(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{HTCLIENT, WM_SETCURSOR};

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_SETCURSOR && (lparam as u32 & 0xFFFF) == HTCLIENT as u32 {
            return 1; // WebView2 manages the cursor in its client area.
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    let Ok(hwnd) = window.hwnd() else {
        warn!("tray-menu hwnd unavailable; cursor hook skipped");
        return;
    };
    let ok = unsafe { SetWindowSubclass(hwnd.0 as HWND, Some(subclass_proc), 1, 0) };
    if ok == 0 {
        warn!("Failed to subclass tray-menu window for cursor handling");
    }
}

#[cfg(not(windows))]
pub(crate) fn hook_tray_menu_cursor(_window: &tauri::WebviewWindow) {}

fn show_tray_menu(app: &AppHandle, position: PhysicalPosition<f64>, icon_rect: Rect) {
    let Some(window) = app.get_webview_window("tray-menu") else {
        warn!("tray-menu window missing; attempting resume recovery");
        recover_after_resume(app);
        return;
    };

    let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
    let (mw, mh) = (MENU_LOGICAL_W * scale, MENU_LOGICAL_H * scale);

    let icon_pos = icon_rect.position.to_physical::<f64>(scale);
    let icon_size = icon_rect.size.to_physical::<f64>(scale);
    let icon = (icon_pos.x, icon_pos.y, icon_size.width, icon_size.height);
    // Fallback if tray reports an empty rect (some Windows builds / post-sleep).
    let icon = if icon.2 > 1.0 && icon.3 > 1.0 {
        icon
    } else {
        (position.x, position.y, 16.0 * scale, 16.0 * scale)
    };

    // Prefer monitor containing the icon (hidden window's current_monitor may be wrong)
    let anchor_x = icon.0 + icon.2 * 0.5;
    let anchor_y = icon.1 + icon.3 * 0.5;
    let work = app
        .available_monitors()
        .ok()
        .into_iter()
        .flatten()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            anchor_x >= pos.x as f64
                && anchor_y >= pos.y as f64
                && anchor_x < pos.x as f64 + size.width as f64
                && anchor_y < pos.y as f64 + size.height as f64
        })
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
        .map(|m| {
            let area = m.work_area();
            (
                area.position.x as f64,
                area.position.y as f64,
                area.size.width as f64,
                area.size.height as f64,
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0 * scale, 1080.0 * scale));

    let (x, y) = anchor_menu_to_tray_icon(icon, (mw, mh), work);
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(
        x.round() as i32,
        y.round() as i32,
    )));
    let _ = window.unminimize();
    let _ = window.show();
    if let Ok(hwnd) = window.hwnd() {
        let _ = crate::clipboard::focus_window(hwnd.0 as isize);
    } else {
        let _ = window.set_focus();
    }
    let _ = app.emit("tray-menu-opened", ());
}

#[cfg(test)]
mod clamp_menu_position_tests {
    use super::{anchor_menu_to_tray_icon, clamp_menu_position};

    #[test]
    fn clamps_to_bottom_right_when_near_edge() {
        let (x, y) =
            clamp_menu_position((1900.0, 1000.0), (260.0, 220.0), (0.0, 0.0, 1920.0, 1080.0));
        assert!(x <= 1920.0 - 260.0 - 8.0);
        assert!(y <= 1080.0 - 220.0 - 8.0);
    }

    #[test]
    fn keeps_pad_from_origin() {
        let (x, y) = clamp_menu_position((0.0, 0.0), (260.0, 220.0), (0.0, 0.0, 1920.0, 1080.0));
        assert_eq!((x, y), (8.0, 8.0));
    }

    #[test]
    fn opens_above_and_right_aligned_to_tray_icon() {
        // Icon near bottom-right of work area (typical Windows tray).
        let icon = (1800.0, 1040.0, 24.0, 24.0);
        let menu = (176.0, 148.0);
        let work = (0.0, 0.0, 1920.0, 1040.0); // work area ends above taskbar
        let (x, y) = anchor_menu_to_tray_icon(icon, menu, work);
        assert!((x - (1800.0 + 24.0 - 176.0)).abs() < 0.1);
        // Preferred above icon, then clamped into work area (8px pad).
        let preferred_y: f64 = 1040.0 - 148.0 - 4.0;
        let max_y: f64 = 1040.0 - 148.0 - 8.0;
        assert!((y - preferred_y.min(max_y)).abs() < 0.1);
    }

    #[test]
    fn opens_below_when_no_room_above() {
        let icon = (100.0, 10.0, 24.0, 24.0);
        let menu = (176.0, 148.0);
        let work = (0.0, 0.0, 1920.0, 1080.0);
        let (_x, y) = anchor_menu_to_tray_icon(icon, menu, work);
        // Not enough space above → open below icon with gap
        assert!((y - (10.0 + 24.0 + 4.0)).abs() < 0.1);
    }
}
