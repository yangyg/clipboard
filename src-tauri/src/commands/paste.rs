//! Paste pipeline commands: write clipboard → focus target → Ctrl+V.
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::warn;

use crate::clipboard;
use crate::media;
use crate::{AppState, Settings};

/// Remember the current foreground app as paste destination (safe no-op if FG is us).
#[tauri::command]
pub async fn capture_paste_target(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let our = window.hwnd().ok().map(|h| h.0 as isize);
        clipboard::set_our_main_hwnd(our);
        clipboard::remember_paste_target(our);
    }
    Ok(())
}

async fn focus_paste_target_on_main_thread(app: &AppHandle, hwnd: isize) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if app
        .run_on_main_thread(move || {
            let ok = clipboard::focus_window(hwnd);
            let _ = tx.send(ok);
        })
        .is_err()
    {
        return clipboard::focus_window(hwnd);
    }
    rx.await.unwrap_or(false)
}

#[tauri::command]
pub async fn paste_record(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    mode: Option<String>,
) -> Result<(), String> {
    // H-5: Read-only preparation OUTSIDE the mutex — reduces lock hold time.
    let settings = match state.db.get_settings() {
        Ok(s) => (*s).clone(),
        Err(e) => {
            warn!("Failed to load settings for paste; using defaults: {}", e);
            Settings::default()
        }
    };
    let auto_close = settings.auto_close_on_paste;
    // Prefer live chrome over DB — matches what the user actually sees.
    let is_floating = app
        .get_webview_window("main")
        .and_then(|w| w.is_always_on_top().ok())
        .unwrap_or(settings.app_mode != "window");

    let our_hwnd = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);
    clipboard::set_our_main_hwnd(our_hwnd);

    let _ = app.emit("paste-focus-lock", true);
    struct PasteFocusUnlock<'a>(&'a AppHandle);
    impl Drop for PasteFocusUnlock<'_> {
        fn drop(&mut self) {
            let _ = self.0.emit("paste-focus-lock", false);
        }
    }
    let _paste_focus_unlock = PasteFocusUnlock(&app);

    let db = state.db.clone();
    let monitor = state.monitor.clone();
    let media_root = state.db.media_root().to_path_buf();

    // Serialize only the critical section: clipboard write → focus → Ctrl+V → restore.
    // H-5: Mutex now guards only the timing-sensitive paste sequence, not the
    // read-only preparation above. This reduces contention when multiple paste
    // requests queue up (e.g. rapid keyboard shortcuts).
    static PASTE_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _paste_guard = PASTE_GATE.lock().await;

    // 1) Write clipboard while we still own the foreground (focus rights intact).
    let outcome = tokio::task::spawn_blocking(move || {
        let record = db.take_record_for_paste(id).map_err(|e| e.to_string())?;
        let Some(r) = record else {
            return Ok::<_, String>(PasteOutcome::Missing);
        };

        monitor
            .read()
            .suppress_self_writes(std::time::Duration::from_millis(1500));

        let wrote = if r.content_type == "image" {
            if let Some(media_path) = r.media_path.as_deref() {
                let abs = crate::security::resolve_media_file(&media_root, media_path)?;
                if clipboard::write_clipboard_png_file(&abs) {
                    true
                } else {
                    let (rgba, w, h) = media::load_image_rgba(&media_root, media_path)?;
                    clipboard::write_clipboard_image(&rgba, w, h)
                }
            } else {
                return Err("该记录的图片文件缺失".into());
            }
        } else {
            match mode.as_deref() {
                Some("plain") => clipboard::write_clipboard_plain(&r.content),
                _ => clipboard::write_clipboard_text(&r.content, r.content_html.as_deref()),
            }
        };
        if !wrote {
            return Err("写入剪贴板失败".into());
        }
        Ok(PasteOutcome::Ready)
    })
    .await
    .map_err(|e| format!("paste task join error: {e}"))??;

    if matches!(outcome, PasteOutcome::Missing) {
        return Ok(());
    }

    // 2) Focus the previous app FIRST (we still have FG privilege as the active window).
    clipboard::track_last_foreign_foreground();
    let mut target = clipboard::resolve_paste_target(our_hwnd);
    if target.is_none() {
        warn!(
            "No paste target yet (panel may have opened before any other app was focused); \
             will retry after yielding foreground"
        );
    }
    let mut focused = false;
    if let Some(hwnd) = target {
        focused = focus_paste_target_on_main_thread(&app, hwnd).await;
    }

    // 3) Yield foreground — do NOT leave FG before focus (that drops FG rights).
    // Floating: hide to tray. Window: minimize to taskbar (hide feels like "closed").
    if is_floating {
        if let Some(hwnd) = our_hwnd {
            clipboard::hide_hwnd(hwnd);
        }
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        let _ = app.emit("toggle-panel", false);
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }

    // After we leave the foreground, Windows may activate the previous app —
    // pick it up if we had no target yet.
    for _ in 0..5 {
        if target.is_some() && focused {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        clipboard::track_last_foreign_foreground();
        if target.is_none() {
            target = clipboard::resolve_paste_target(our_hwnd);
        }
        if let Some(hwnd) = target {
            if !focused {
                focused = focus_paste_target_on_main_thread(&app, hwnd).await;
            }
        }
        if focused || clipboard::foreground_is_pasteable(our_hwnd) {
            break;
        }
    }

    let can_paste = focused || clipboard::foreground_is_pasteable(our_hwnd);
    if can_paste {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        clipboard::simulate_paste_keys();
    } else {
        warn!(
            "Paste target unavailable (hwnd={:?}); clipboard updated, skipped Ctrl+V",
            target
        );
    }

    // 5) Re-show only when keep-open; never steal focus back after a successful paste.
    // auto_close + floating → stay hidden; auto_close + window → stay minimized.
    if !auto_close {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            // Deliberately no set_focus — leave the target app active.
        }
        if is_floating {
            let _ = app.emit("toggle-panel", true);
        }
    }

    Ok(())
}

enum PasteOutcome {
    Missing,
    Ready,
}
