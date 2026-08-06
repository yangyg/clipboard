//! Settings persistence, autostart, capture-pause, and window mode commands.
use std::sync::atomic::Ordering as AtomicOrdering;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tracing::{info, warn};

use crate::window;
use crate::{AppState, Settings};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let arc = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok((*arc).clone())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    let previous = state.db.get_settings().map_err(|e| e.to_string())?;
    let autostart_changed = settings.auto_start != previous.auto_start;
    let shortcut_changed = settings.global_shortcut != previous.global_shortcut;

    // Window sizes are only written by resize persistence — never let frontend
    // autosave (stale/zero defaults) wipe remembered dimensions.
    settings.floating_width = previous.floating_width;
    settings.floating_height = previous.floating_height;
    settings.window_width = previous.window_width;
    settings.window_height = previous.window_height;

    if autostart_changed {
        apply_autostart(&app, settings.auto_start)?;
    }

    if settings.retention_days != previous.retention_days {
        state
            .db
            .cleanup_retention(settings.retention_days)
            .map_err(|e| e.to_string())?;
    }
    if let Err(e) = state.db.save_settings(&settings) {
        if autostart_changed {
            if let Err(revert_err) = apply_autostart(&app, previous.auto_start) {
                warn!(
                    "Failed to revert autostart after settings save error: {}",
                    revert_err
                );
            }
        }
        return Err(e.to_string());
    }

    if shortcut_changed {
        if let Err(e) = crate::apply_global_shortcut(&app, &settings.global_shortcut) {
            warn!("Failed to apply new global shortcut: {}", e);
            // Persist succeeded; surface so UI can reload / warn
            return Err(e);
        }
    }

    if settings.panel_radius != previous.panel_radius {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window::apply_window_round_corners(&window, settings.panel_radius);
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
pub async fn set_capture_paused(state: State<'_, AppState>, paused: bool) -> Result<(), String> {
    *state.capture_paused.write() = paused;
    info!("Capture paused: {}", paused);
    Ok(())
}

// === App Mode / Window Commands ===

#[tauri::command]
pub async fn switch_app_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    let is_window = mode == "window";
    let settings = match app
        .try_state::<AppState>()
        .and_then(|s| s.db.get_settings().ok())
    {
        Some(s) => (*s).clone(),
        None => {
            warn!("Failed to load settings for mode switch; using defaults");
            Settings::default()
        }
    };
    let (w, h) = window::resolve_panel_size(&window, &settings, is_window);
    let (min_w, min_h, _, _) = window::mode_size_bounds(is_window);
    window.set_decorations(false).map_err(|e| e.to_string())?;
    let _ = window.set_shadow(false);
    window
        .set_always_on_top(!is_window)
        .map_err(|e| e.to_string())?;
    window
        .set_skip_taskbar(!is_window)
        .map_err(|e| e.to_string())?;
    // Both modes resizable so remembered size can be adjusted
    window.set_resizable(true).map_err(|e| e.to_string())?;
    let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
        min_w, min_h,
    ))));
    // Cancel pending resize-save so programmatic set_size doesn't overwrite
    // the other mode's remembered size while app_mode is mid-switch.
    window::SIZE_SAVE_GEN.fetch_add(1, AtomicOrdering::Relaxed);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
    // Re-apply rounded region after size change
    let _ = window::apply_window_round_corners(&window, settings.panel_radius);
    info!("App mode switched to: {} (size {}x{})", mode, w, h);
    Ok(())
}

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
