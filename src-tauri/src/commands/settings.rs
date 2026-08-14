//! Settings persistence, autostart, capture-pause, and window commands.
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tracing::{info, warn};

use crate::window;
use crate::{AppState, Settings};

use super::spawn_db;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let db = state.db.clone();
    let arc = spawn_db(move || db.get_settings()).await?;
    Ok((*arc).clone())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    let db = state.db.clone();
    let previous = spawn_db(move || db.get_settings()).await?;
    let autostart_changed = settings.auto_start != previous.auto_start;
    let shortcut_changed = settings.global_shortcut != previous.global_shortcut;

    // Window sizes are only written by resize persistence — never let frontend
    // autosave (stale/zero defaults) wipe remembered dimensions.
    settings.window_width = previous.window_width;
    settings.window_height = previous.window_height;

    if autostart_changed {
        apply_autostart(&app, settings.auto_start)?;
    }

    // Bind the new shortcut *before* persisting so a registration failure
    // cannot leave DB and runtime out of sync.
    if shortcut_changed {
        crate::apply_global_shortcut(&app, &settings.global_shortcut)?;
    }

    if settings.retention_days != previous.retention_days {
        let db = state.db.clone();
        let days = settings.retention_days;
        spawn_db(move || db.cleanup_retention(days)).await?;
    }
    let db = state.db.clone();
    let settings_for_db = settings.clone();
    if let Err(e) = spawn_db(move || db.save_settings(&settings_for_db)).await {
        if autostart_changed {
            if let Err(revert_err) = apply_autostart(&app, previous.auto_start) {
                warn!(
                    "Failed to revert autostart after settings save error: {}",
                    revert_err
                );
            }
        }
        if shortcut_changed {
            if let Err(revert_err) = crate::apply_global_shortcut(&app, &previous.global_shortcut) {
                warn!(
                    "Failed to revert global shortcut after settings save error: {}",
                    revert_err
                );
            }
        }
        return Err(e);
    }

    let chrome_changed = settings.panel_radius != previous.panel_radius
        || settings.always_on_top != previous.always_on_top
        || settings.enable_blur != previous.enable_blur;
    if chrome_changed {
        if let Some(window) = app.get_webview_window("main") {
            window::apply_window_chrome(&window, &settings);
        }
    }
    let _ = app.emit("settings-updated", ());
    Ok(())
}

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

#[tauri::command]
pub async fn set_capture_paused(
    app: AppHandle,
    state: State<'_, AppState>,
    paused: bool,
) -> Result<(), String> {
    *state.capture_paused.write() = paused;
    info!("Capture paused: {}", paused);
    let _ = app.emit("capture-paused", paused);
    Ok(())
}

// === Window Commands ===

#[tauri::command]
pub async fn set_window_corner_radius(app: AppHandle, radius: i32) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    window::apply_window_round_corners(&window, radius)
}

/// Enable/disable native DWM frosted-glass backdrop (acrylic) on all app windows.
/// Applied by the frontend from the 毛玻璃 toggle; the same command is re-run
/// when the tray-menu window is (re)created so it follows settings too.
#[tauri::command]
pub async fn set_window_backdrop(app: AppHandle, enabled: bool) -> Result<(), String> {
    for label in ["main", "tray-menu"] {
        if let Some(window) = app.get_webview_window(label) {
            window::apply_window_backdrop(&window, enabled)?;
        }
    }
    Ok(())
}
