//! System tray icon, menu and event wiring. Extracted from `lib.rs`;
//! behaviour unchanged.

use std::sync::Arc;

use parking_lot::RwLock;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tracing::info;

/// Build the system tray icon + menu and register its event handlers.
pub(crate) fn build_tray(
    app: &tauri::App,
    capture_paused: Arc<RwLock<bool>>,
) -> tauri::Result<()> {
    let capture_paused_menu = capture_paused;

    let show_item = MenuItemBuilder::with_id("show", "📋 打开面板").build(app)?;
    let pause_item = MenuItemBuilder::with_id("pause", "⏸ 暂停捕获").build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "⚙ 设置").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "❌ 退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&pause_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("剪贴板管理")
        .menu(&menu)
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show" => {
                    crate::show_main_panel(app);
                    info!("Tray menu: show panel");
                }
                "pause" => {
                    let next = !*capture_paused_menu.read();
                    *capture_paused_menu.write() = next;
                    app.emit("capture-paused", next).ok();
                    info!("Tray menu: capture paused = {}", next);
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let our = window.hwnd().ok().map(|h| h.0 as isize);
                        crate::clipboard::remember_paste_target(our);
                        window.show().ok();
                        window.set_focus().ok();
                        app.emit("open-settings", ()).ok();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        window.hide().ok();
                        app.emit("toggle-panel", false).ok();
                    } else {
                        crate::show_main_panel(app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
