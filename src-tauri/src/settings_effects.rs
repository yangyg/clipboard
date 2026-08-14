//! OS side effects that follow a **user** settings save (autostart, shortcut,
//! window chrome). Window geometry, device identity, and WebDAV sync stamps
//! persist through `ClipboardDb` helpers and must not run this path.

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tracing::{info, warn};

use crate::window;
use crate::Settings;

/// Enable or disable OS autostart. No-op when the registry already matches.
pub(crate) fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();

    // No-op when OS state already matches — disable() errors with
    // ERROR_FILE_NOT_FOUND if the Run key value was never created.
    match manager.is_enabled() {
        Ok(currently) if currently == enabled => return Ok(()),
        Ok(_) => {}
        Err(e) => warn!("Could not read autostart state: {}", e),
    }

    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => {
            info!("Autostart {}", if enabled { "enabled" } else { "disabled" });
            Ok(())
        }
        Err(e) if !enabled && is_autostart_already_cleared(&e) => {
            // Registry value already absent — treat as disabled.
            Ok(())
        }
        Err(e) => {
            let msg = format!(
                "Failed to {} autostart: {}",
                if enabled { "enable" } else { "disable" },
                e
            );
            warn!("{}", msg);
            Err(msg)
        }
    }
}

fn is_autostart_already_cleared(err: &impl std::fmt::Display) -> bool {
    let s = err.to_string();
    s.contains("os error 2")
        || s.contains("找不到指定的文件")
        || s.to_ascii_lowercase().contains("not found")
}

/// Revert autostart / shortcut after a failed `save_settings` persist.
pub(crate) fn revert_os_side_effects(
    app: &AppHandle,
    previous: &Settings,
    autostart_changed: bool,
    shortcut_changed: bool,
) {
    if autostart_changed {
        if let Err(revert_err) = apply_autostart(app, previous.auto_start) {
            warn!(
                "Failed to revert autostart after settings save error: {}",
                revert_err
            );
        }
    }
    if shortcut_changed {
        if let Err(revert_err) = crate::apply_global_shortcut(app, &previous.global_shortcut) {
            warn!(
                "Failed to revert global shortcut after settings save error: {}",
                revert_err
            );
        }
    }
}

/// Apply always-on-top / corners / blur when those fields actually changed.
pub(crate) fn apply_window_chrome_if_changed(
    app: &AppHandle,
    previous: &Settings,
    settings: &Settings,
) {
    let chrome_changed = settings.panel_radius != previous.panel_radius
        || settings.always_on_top != previous.always_on_top
        || settings.enable_blur != previous.enable_blur;
    if chrome_changed {
        if let Some(window) = app.get_webview_window("main") {
            window::apply_window_chrome(&window, settings);
        }
    }
}
