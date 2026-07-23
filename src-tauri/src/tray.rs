//! System tray icon and event wiring. Right-click shows the custom
//! `tray-menu` window anchored to the tray icon; left-click toggles the main panel.

use std::sync::Arc;

use parking_lot::RwLock;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, PhysicalPosition, Position, Rect};

/// Logical size of the tray-menu window (must match `tauri.conf.json`).
const MENU_LOGICAL_W: f64 = 176.0;
/// Initial / fallback height; frontend resizes to content on open.
const MENU_LOGICAL_H: f64 = 148.0;
const MENU_GAP: f64 = 4.0;

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
    app: &tauri::App,
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
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            window.hide().ok();
                            app.emit("toggle-panel", false).ok();
                        } else {
                            crate::show_main_panel(app);
                        }
                    }
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

fn show_tray_menu(app: &tauri::AppHandle, position: PhysicalPosition<f64>, icon_rect: Rect) {
    let Some(window) = app.get_webview_window("tray-menu") else {
        return;
    };

    let scale = window.scale_factor().unwrap_or(1.0);
    let (mw, mh) = (MENU_LOGICAL_W * scale, MENU_LOGICAL_H * scale);

    let icon_pos = icon_rect.position.to_physical::<f64>(scale);
    let icon_size = icon_rect.size.to_physical::<f64>(scale);
    let icon = (icon_pos.x, icon_pos.y, icon_size.width, icon_size.height);
    // Fallback if tray reports an empty rect (some Windows builds).
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
    let _ = window.show();
    let _ = window.set_focus();
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
