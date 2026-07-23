//! System tray icon and event wiring. Right-click shows the custom
//! `tray-menu` window near the cursor; left-click toggles the main panel.

use std::sync::Arc;

use parking_lot::RwLock;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

/// Clamp menu top-left so the menu stays inside the work area (physical px).
pub(crate) fn clamp_menu_position(
    click: (f64, f64),
    menu_size: (f64, f64),
    work: (f64, f64, f64, f64), // x, y, w, h
) -> (f64, f64) {
    let (cx, cy) = click;
    let (mw, mh) = menu_size;
    let (wx, wy, ww, wh) = work;
    let pad = 8.0;
    let max_x = wx + ww - mw - pad;
    let max_y = wy + wh - mh - pad;
    let x = cx.min(max_x).max(wx + pad);
    let y = cy.min(max_y).max(wy + pad);
    (x, y)
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
                    ..
                } => {
                    show_tray_menu(app, position);
                }
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

fn show_tray_menu(app: &tauri::AppHandle, position: tauri::PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window("tray-menu") else {
        return;
    };

    let scale = window.scale_factor().unwrap_or(1.0);
    let (mw, mh) = (260.0 * scale, 220.0 * scale);

    // Prefer monitor containing the click (hidden window's current_monitor may be wrong)
    let work = app
        .available_monitors()
        .ok()
        .into_iter()
        .flatten()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            let x = position.x;
            let y = position.y;
            x >= pos.x as f64
                && y >= pos.y as f64
                && x < pos.x as f64 + size.width as f64
                && y < pos.y as f64 + size.height as f64
        })
        .or_else(|| window.current_monitor().ok().flatten())
        .map(|m| {
            // Use work area (excludes taskbar) per design spec.
            let area = m.work_area();
            (
                area.position.x as f64,
                area.position.y as f64,
                area.size.width as f64,
                area.size.height as f64,
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0 * scale, 1080.0 * scale));

    let (x, y) = clamp_menu_position((position.x, position.y), (mw, mh), work);
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x.round() as i32,
        y.round() as i32,
    )));
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("tray-menu-opened", ());
}

#[cfg(test)]
mod clamp_menu_position_tests {
    use super::clamp_menu_position;

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
}
