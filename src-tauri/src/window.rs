//! Window geometry, adaptive sizing, rounded-corner clipping and resize
//! persistence. Extracted from `lib.rs`; behaviour unchanged.

use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use tauri::Manager;
use tracing::{info, warn};

use crate::{AppState, Settings};

fn monitor_work_area_logical(window: &tauri::WebviewWindow) -> (f64, f64) {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .map(|m| {
            let scale = m.scale_factor().max(0.5);
            let area = m.work_area();
            let w = (area.size.width as f64 / scale).max(320.0);
            let h = (area.size.height as f64 / scale).max(320.0);
            (w, h)
        })
        .unwrap_or((1920.0, 1080.0))
}

/// Logical panel size from the current (or primary) monitor work area.
/// Floating ≈ 40%×65% of screen; window ≈ 55%×72%; clamped so small laptops
/// stay usable and large 4K screens don't open a huge panel.
fn adaptive_panel_size(window: &tauri::WebviewWindow, is_window_mode: bool) -> (f64, f64) {
    let (frac_w, frac_h, min_w, min_h, max_w, max_h) = if is_window_mode {
        // Window mode needs ≥780 so SideBar(200)+List(280)+Preview(280)+resizers fit.
        (0.55, 0.72, 780.0, 520.0, 1280.0, 900.0)
    } else {
        (0.40, 0.65, 480.0, 480.0, 800.0, 780.0)
    };

    let (screen_w, screen_h) = monitor_work_area_logical(window);

    let w = (screen_w * frac_w)
        .clamp(min_w, max_w)
        .min((screen_w - 32.0).max(min_w));
    let h = (screen_h * frac_h)
        .clamp(min_h, max_h)
        .min((screen_h - 32.0).max(min_h));
    (w.round(), h.round())
}

pub(crate) fn mode_size_bounds(is_window_mode: bool) -> (f64, f64, f64, f64) {
    if is_window_mode {
        (780.0, 400.0, 1600.0, 1200.0)
    } else {
        (400.0, 400.0, 1000.0, 900.0)
    }
}

/// Prefer remembered size when valid; otherwise adaptive. Always clamp to work area.
pub(crate) fn resolve_panel_size(
    window: &tauri::WebviewWindow,
    settings: &Settings,
    is_window_mode: bool,
) -> (f64, f64) {
    let (saved_w, saved_h) = if is_window_mode {
        (settings.window_width, settings.window_height)
    } else {
        (settings.floating_width, settings.floating_height)
    };
    let (min_w, min_h, max_w, max_h) = mode_size_bounds(is_window_mode);
    let (screen_w, screen_h) = monitor_work_area_logical(window);

    if saved_w >= min_w as i32 && saved_h >= min_h as i32 {
        let w = (saved_w as f64)
            .clamp(min_w, max_w)
            .min((screen_w - 32.0).max(min_w));
        let h = (saved_h as f64)
            .clamp(min_h, max_h)
            .min((screen_h - 32.0).max(min_h));
        return (w.round(), h.round());
    }
    adaptive_panel_size(window, is_window_mode)
}

fn persist_current_window_size(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    // Don't remember maximized geometry as the restored size
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
    let w = ((size.width as f64) / scale).round() as i32;
    let h = ((size.height as f64) / scale).round() as i32;
    if w < 200 || h < 200 {
        return;
    }

    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(settings_arc) = state.db.get_settings() else {
        return;
    };
    // Cold path (debounced resize): clone inner Settings for mutation.
    let mut settings = (*settings_arc).clone();
    // Prefer live window chrome over DB app_mode (mode switch may not be saved yet).
    let is_window = !window
        .is_always_on_top()
        .unwrap_or(settings.app_mode == "window");
    let (min_w, min_h, _, _) = mode_size_bounds(is_window);
    if (w as f64) < min_w || (h as f64) < min_h {
        return;
    }

    if is_window {
        if settings.window_width == w && settings.window_height == h {
            return;
        }
        settings.window_width = w;
        settings.window_height = h;
    } else {
        if settings.floating_width == w && settings.floating_height == h {
            return;
        }
        settings.floating_width = w;
        settings.floating_height = h;
    }
    if let Err(e) = state.db.save_settings(&settings) {
        warn!("Failed to persist window size: {}", e);
    } else {
        let mode = if is_window { "window" } else { "floating" };
        info!("Remembered {} size {}x{}", mode, w, h);
    }
}

/// Debounce resize → settings write (Resized fires continuously while dragging).
pub(crate) static SIZE_SAVE_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn schedule_persist_window_size(app: tauri::AppHandle) {
    let gen = SIZE_SAVE_GEN.fetch_add(1, AtomicOrdering::Relaxed) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if SIZE_SAVE_GEN.load(AtomicOrdering::Relaxed) != gen {
            return;
        }
        if let Some(window) = app.get_webview_window("main") {
            persist_current_window_size(&app, &window);
        }
    });
}

/// Clip the HWND to a rounded rect so corners are not rectangular (and not black).
/// `radius` is logical CSS px from 面板外观 → 圆角大小; scaled to physical pixels.
pub(crate) fn apply_window_round_corners(
    window: &tauri::WebviewWindow,
    radius_logical: i32,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};

        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let w = size.width as i32;
        let h = size.height as i32;
        let scale = window.scale_factor().unwrap_or(1.0);
        let radius = ((radius_logical.max(0) as f64) * scale).round() as i32;

        // Clear region → rectangular window
        if radius <= 0 {
            let ok = unsafe { SetWindowRgn(hwnd.0 as _, std::ptr::null_mut(), 1) };
            if ok == 0 {
                return Err("SetWindowRgn(null) failed".into());
            }
            return Ok(());
        }

        // Ellipse width/height = 2 * corner radius (Win32 convention)
        let ellipse = (radius * 2).max(1);
        // +1 on bottom-right is required by CreateRoundRectRgn (exclusive edge)
        let hrgn = unsafe { CreateRoundRectRgn(0, 0, w + 1, h + 1, ellipse, ellipse) };
        if hrgn.is_null() {
            return Err("CreateRoundRectRgn failed".into());
        }
        let ok = unsafe { SetWindowRgn(hwnd.0 as _, hrgn, 1) };
        if ok == 0 {
            unsafe {
                DeleteObject(hrgn);
            }
            return Err("SetWindowRgn failed".into());
        }
        // Ownership of hrgn transferred to the system on success
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, radius_logical);
        Ok(())
    }
}

/// Windows native frosted-glass backdrop (acrylic) behind a transparent window.
/// CSS `backdrop-filter` cannot blur the OS desktop behind a transparent WebView2
/// window (there is no web content to sample), so 毛玻璃 must come from DWM.
/// `Effect::Acrylic` maps to window-vibrancy: DWMSBT_TRANSIENTWINDOW on Win11,
/// SetWindowCompositionAttribute(ACCENT_ENABLE_ACRYLICBLURBEHIND) on Win10.
pub(crate) fn apply_window_backdrop(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri::window::{Effect, EffectsBuilder};

        let effects = if enabled {
            Some(EffectsBuilder::new().effect(Effect::Acrylic).build())
        } else {
            None
        };
        window
            .as_ref()
            .window()
            .set_effects(effects)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, enabled);
        Ok(())
    }
}
