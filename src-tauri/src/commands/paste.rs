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

/// Write the record to the clipboard, then try to send Ctrl+V to the paste target.
///
/// Returns `Ok(true)` when key injection ran, `Ok(false)` when the clipboard was
/// updated but no valid target was focused (caller should tell the user to paste
/// manually). Missing/trashed records and clipboard write failures are `Err`.
#[tauri::command]
pub async fn paste_record(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    mode: Option<String>,
) -> Result<bool, String> {
    // H-5: Read-only preparation OUTSIDE the mutex — reduces lock hold time.
    let auto_close = match state.db.get_settings() {
        Ok(s) => s.auto_close_on_paste,
        Err(e) => {
            warn!("Failed to load settings for paste; using defaults: {}", e);
            Settings::default().auto_close_on_paste
        }
    };

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
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let record = db.take_record_for_paste(id).map_err(|e| e.to_string())?;
        let Some(r) = record else {
            return Err("记录不存在或已在回收站".into());
        };

        monitor
            .read()
            .suppress_self_writes(std::time::Duration::from_millis(1500));

        let wrote = if r.content_type == "image" {
            if let Some(media_path) = r.media_path.as_deref() {
                let abs = crate::security::resolve_media_file(&media_root, media_path)?;
                if clipboard::write_clipboard_png_file(&abs) {
                    // PNG-only format carries no CF_BITMAP/CF_UNICODETEXT, so the
                    // monitor emits nothing for it — no baseline sync needed.
                    true
                } else {
                    let (rgba, w, h) = media::load_image_rgba(&media_root, media_path)?;
                    let ok = clipboard::write_clipboard_image(&rgba, w, h);
                    if ok {
                        // Absorb the post-suppression re-read of our own write;
                        // otherwise it re-captures with the paste-target window
                        // as source.
                        monitor.read().mark_image_written(
                            &clipboard::image_quick_fingerprint_rgba(&rgba, w, h),
                        );
                    }
                    ok
                }
            } else {
                return Err("该记录的图片文件缺失".into());
            }
        } else {
            let ok = match mode.as_deref() {
                Some("plain") => clipboard::write_clipboard_plain(&r.content),
                _ => clipboard::write_clipboard_text(&r.content, r.content_html.as_deref()),
            };
            if ok {
                monitor.read().mark_text_written(&r.content);
            }
            ok
        };
        if !wrote {
            return Err("写入剪贴板失败".into());
        }
        db.bump_copy_count(r.id).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("paste task join error: {e}"))??;

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
    // Window mode: minimize to taskbar (hide feels like "closed").
    if let Some(window) = app.get_webview_window("main") {
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
    // auto_close → stay minimized.
    if !auto_close {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            // Deliberately no set_focus — leave the target app active.
        }
    }

    Ok(can_paste)
}
