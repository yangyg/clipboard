//! Tray-menu window state + action commands.
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

use super::spawn_db;

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

/// Side effects the tray-menu action is allowed to take against the main panel.
///
/// `OpenSettings` must not activate the home view first: `show_main_panel`
/// paints the list, and WebView2 keeps that compositor frame until the next
/// click — so Settings only appears after the user clicks the home page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayMenuIntent {
    ShowMain,
    TogglePause,
    OpenSettings,
    Quit,
}

pub(crate) fn tray_menu_intent(action: &str) -> Result<TrayMenuIntent, String> {
    match action {
        "show" => Ok(TrayMenuIntent::ShowMain),
        "pause" => Ok(TrayMenuIntent::TogglePause),
        "settings" => Ok(TrayMenuIntent::OpenSettings),
        "quit" => Ok(TrayMenuIntent::Quit),
        other => Err(format!("unknown tray action: {other}")),
    }
}

#[tauri::command]
pub async fn get_tray_menu_state(state: State<'_, AppState>) -> Result<TrayMenuState, String> {
    let db = state.db.clone();
    let settings = spawn_db(move || db.get_settings()).await?;
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

    match tray_menu_intent(&action) {
        Ok(TrayMenuIntent::ShowMain) => {
            crate::show_main_panel(&app);
            hide_tray();
        }
        Ok(TrayMenuIntent::TogglePause) => {
            hide_tray();
            let next = !*state.capture_paused.read();
            *state.capture_paused.write() = next;
            let _ = app.emit("capture-paused", next);
        }
        Ok(TrayMenuIntent::OpenSettings) => {
            // Emit only. The frontend swaps to SettingsWindow while the main
            // window is still hidden, then shows/focuses itself. Activating
            // here would paint the home list first (see TrayMenuIntent).
            // Do not hide the tray either — hiding first drops FG rights;
            // the tray-menu window already hides itself on focus loss.
            let _ = app.emit("open-settings", ());
        }
        Ok(TrayMenuIntent::Quit) => {
            hide_tray();
            app.exit(0);
        }
        Err(e) => {
            hide_tray();
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{tray_menu_intent, TrayMenuIntent};

    #[test]
    fn settings_does_not_show_main_panel_first() {
        assert_eq!(
            tray_menu_intent("settings").unwrap(),
            TrayMenuIntent::OpenSettings
        );
        assert_ne!(
            tray_menu_intent("settings").unwrap(),
            TrayMenuIntent::ShowMain
        );
    }

    #[test]
    fn known_actions_map_to_intents() {
        assert_eq!(tray_menu_intent("show").unwrap(), TrayMenuIntent::ShowMain);
        assert_eq!(
            tray_menu_intent("pause").unwrap(),
            TrayMenuIntent::TogglePause
        );
        assert_eq!(tray_menu_intent("quit").unwrap(), TrayMenuIntent::Quit);
        assert!(tray_menu_intent("nope").is_err());
    }
}
