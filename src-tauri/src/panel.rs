//! Main-panel show/toggle, global shortcut, ignore-app matching, and the
//! light clipboard-changed IPC payload.

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{info, warn};

use crate::clipboard::{
    focus_window, is_foreground_hwnd, remember_paste_target, set_our_main_hwnd,
};
use crate::ClipboardRecord;

/// Remember the previous foreground app, then show + focus the main panel.
pub(crate) fn show_main_panel(app: &tauri::AppHandle) {
    let perf_start = std::time::Instant::now();
    if let Some(window) = app.get_webview_window("main") {
        let our = window.hwnd().ok().map(|h| h.0 as isize);
        set_our_main_hwnd(our);
        remember_paste_target(our);
        let _ = window.unminimize();
        let _ = window.show();
        if let Some(hwnd) = our {
            let _ = focus_window(hwnd);
        } else {
            let _ = window.set_focus();
        }
        let _ = app.emit("toggle-panel", true);
    }
    crate::perf::log_elapsed("panel_show", perf_start);
    // Fallback trigger for the startup history import (also fires via
    // WindowEvent::Focused): the panel is about to be foreground now, so WinRT
    // clipboard-history access will succeed.
    #[cfg(windows)]
    crate::win_history::maybe_start_once(app);
}

/// Toggle main panel. Minimized windows count as "hidden" — restore + focus
/// instead of hide (Windows keeps WS_VISIBLE while minimized).
/// Visible but not foreground → bring to front (tray / shortcut expect activate).
pub(crate) fn toggle_main_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let minimized = window.is_minimized().unwrap_or(false);
    if minimized {
        show_main_panel(app);
        return;
    }
    if window.is_visible().unwrap_or(false) {
        let in_foreground = window
            .hwnd()
            .ok()
            .map(|h| is_foreground_hwnd(h.0 as isize))
            .unwrap_or(false)
            || window.is_focused().unwrap_or(false);
        if in_foreground {
            let _ = window.hide();
            let _ = app.emit("toggle-panel", false);
        } else {
            show_main_panel(app);
        }
    } else {
        show_main_panel(app);
    }
}

pub(crate) fn apply_global_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    let shortcut = shortcut.trim();
    if shortcut.is_empty() {
        return Err("快捷键不能为空".into());
    }
    // Unregister any previously registered shortcuts for this app instance
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister shortcuts before rebind: {}", e);
    }
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_main_panel(app);
            }
        })
        .map_err(|e| e.to_string())?;
    info!("Registered global shortcut: {}", shortcut);
    Ok(())
}

/// Reference matcher implementation (kept for unit tests). The production
/// capture path uses `ClipboardDb::is_ignored_app`, which caches the
/// lowercase pattern set per settings snapshot and applies identical rules.
#[cfg(test)]
fn is_ignored_app(source_app: &str, ignored: &[String]) -> bool {
    if source_app.is_empty() || ignored.is_empty() {
        return false;
    }
    let app_lower = source_app.to_lowercase();
    let basename = source_app
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_app)
        .to_lowercase();
    let basename_noext = basename.strip_suffix(".exe").unwrap_or(&basename);

    ignored.iter().any(|pat| {
        let p = pat.trim().to_lowercase();
        if p.is_empty() {
            return false;
        }
        let p_noext = p.strip_suffix(".exe").unwrap_or(p.as_str());
        basename_noext == p_noext || app_lower == p
    })
}

/// Strip HTML and truncate content for clipboard-changed IPC (list stays light).
pub(crate) fn list_ipc_payload(mut r: ClipboardRecord) -> ClipboardRecord {
    r.content_html = None;
    let full_len = r
        .content_len
        .unwrap_or_else(|| r.content.chars().count() as i32);
    r.content_len = Some(full_len);
    const MAX: usize = 400;
    if (full_len as usize) > MAX {
        r.content = r.content.chars().take(MAX).collect();
    }
    r
}

#[cfg(test)]
mod ignored_app_tests {
    use super::is_ignored_app;

    fn ignored(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn matches_exe_basename_with_or_without_extension() {
        let list = ignored(&["1Password.exe"]);
        assert!(is_ignored_app(
            "C:\\Program Files\\1Password\\1Password.exe",
            &list
        ));
        let noext = ignored(&["chrome"]);
        assert!(is_ignored_app(
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            &noext
        ));
    }

    #[test]
    fn does_not_substring_match_unrelated_apps() {
        // Regression: `contains` made pattern "git" match any path containing it.
        let list = ignored(&["git"]);
        assert!(!is_ignored_app(
            "C:\\Users\\me\\AppData\\Roaming\\digit\\app.exe",
            &list
        ));
        assert!(!is_ignored_app("C:\\Windows\\System32\\notepad.exe", &list));
    }

    #[test]
    fn matches_full_path_when_entered() {
        let list = ignored(&["C:\\Tools\\Keepass\\keepassxc.exe"]);
        assert!(is_ignored_app("C:\\Tools\\Keepass\\keepassxc.exe", &list));
        assert!(!is_ignored_app("C:\\Tools\\Other\\keepassxc.exe", &list));
    }
}
