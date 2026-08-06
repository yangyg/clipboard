//! Tray-menu window state + action commands.
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

#[derive(serde::Serialize, Clone)]
pub struct TrayMenuState {
    pub paused: bool,
    pub theme: String,
    pub enable_blur: bool,
    pub blur_strength: i32,
    pub enable_animation: bool,
    pub panel_opacity: i32,
    pub language: String,
}

#[tauri::command]
pub async fn get_tray_menu_state(state: State<'_, AppState>) -> Result<TrayMenuState, String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok(TrayMenuState {
        paused: *state.capture_paused.read(),
        theme: settings.theme.clone(),
        enable_blur: settings.enable_blur,
        blur_strength: settings.blur_strength,
        enable_animation: settings.enable_animation,
        panel_opacity: settings.panel_opacity,
        language: settings.language.clone(),
    })
}

#[tauri::command]
pub async fn tray_menu_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
) -> Result<(), String> {
    // Hide tray-menu after activating main when possible — hiding first drops
    // Windows foreground rights and set_focus on main often fails.
    let hide_tray = || {
        if let Some(w) = app.get_webview_window("tray-menu") {
            let _ = w.hide();
        }
    };

    match action.as_str() {
        "show" => {
            crate::show_main_panel(&app);
            hide_tray();
        }
        "pause" => {
            hide_tray();
            let next = !*state.capture_paused.read();
            *state.capture_paused.write() = next;
            let _ = app.emit("capture-paused", next);
        }
        "settings" => {
            crate::show_main_panel(&app);
            hide_tray();
            let _ = app.emit("open-settings", ());
        }
        "quit" => {
            hide_tray();
            app.exit(0);
        }
        _ => {
            hide_tray();
            return Err(format!("unknown tray action: {action}"));
        }
    }
    Ok(())
}
