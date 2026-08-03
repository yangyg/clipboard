//! All `#[tauri::command]` handlers plus autostart helpers. Extracted from
//! `lib.rs`; behaviour unchanged. Cross-module helpers are referenced via
//! fully-qualified `crate::` paths.

use std::sync::atomic::Ordering as AtomicOrdering;

use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tracing::{info, warn};

use crate::{clipboard, media};
use crate::{
    require_feature, AppState, ClipboardRecord, FeatureId, RecordsPage, SearchResult, Settings,
    StatsData, TagInfo,
};

fn settings_features(state: &State<'_, AppState>) -> Result<crate::FeatureFlags, String> {
    let s = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok(s.features.clone())
}

/// Upper bound for page-size IPC args — a compromised webview must not be able
/// to materialize every record (incl. sensitive) in a single call.
const MAX_PAGE_SIZE: i32 = 200;
/// Upper bound for batch id args, keeps placeholders / SQL bounded.
const MAX_BATCH_IDS: usize = 1000;

fn cap_ids(ids: Vec<i64>) -> Vec<i64> {
    ids.into_iter().take(MAX_BATCH_IDS).collect()
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_records(
    state: State<'_, AppState>,
    limit: Option<i32>,
    offset: Option<i32>,
    trashed: Option<bool>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
    tag: Option<String>,
    sort: Option<String>,
    before_pinned: Option<i32>,
    before_updated_at: Option<String>,
    before_id: Option<i64>,
) -> Result<RecordsPage, String> {
    // Cleanup runs on the periodic thread — keep list reads off the hot path.
    // Bound `limit` so a compromised webview can't materialize every record.
    let limit = limit.unwrap_or(60).clamp(1, MAX_PAGE_SIZE);
    let offset = offset.unwrap_or(0).max(0);
    let include_tags = settings_features(&state)?.tags;
    let records = state
        .db
        .get_records(
            limit,
            offset,
            trashed.unwrap_or(false),
            content_type.as_deref(),
            favorites_only.unwrap_or(false),
            tag.as_deref(),
            sort.as_deref(),
            before_pinned,
            before_updated_at.as_deref(),
            before_id,
            include_tags,
        )
        .map_err(|e| e.to_string())?;
    let has_more = records.len() as i32 >= limit;
    Ok(RecordsPage { records, has_more })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn search_records(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
    offset: Option<i32>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
    tag: Option<String>,
    sort: Option<String>,
) -> Result<SearchResult, String> {
    let start = std::time::Instant::now();
    let limit = limit.unwrap_or(60).clamp(1, MAX_PAGE_SIZE);
    let offset = offset.unwrap_or(0).max(0);
    let include_tags = settings_features(&state)?.tags;
    let records = state
        .db
        .search_records(
            &query,
            limit,
            offset,
            content_type.as_deref(),
            favorites_only.unwrap_or(false),
            tag.as_deref(),
            sort.as_deref(),
            include_tags,
        )
        .map_err(|e| e.to_string())?;
    let has_more = records.len() as i32 >= limit;
    // `total` is this page's length (not a global hit count) — kept for API compat.
    let total = records.len();
    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(SearchResult {
        records,
        total,
        query,
        elapsed_ms,
        has_more,
    })
}

#[tauri::command]
pub async fn get_record(state: State<'_, AppState>, id: i64) -> Result<Option<ClipboardRecord>, String> {
    state.db.get_record(id).map_err(|e| e.to_string())
}

/// Open a record's media file with the OS default app (Photos, etc.).
#[tauri::command]
pub async fn open_record_media(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let record = state.db.get_record(id).map_err(|e| e.to_string())?;
    let Some(r) = record else {
        return Err("记录不存在".into());
    };
    let Some(rel) = r.media_path.as_deref().filter(|s| !s.is_empty()) else {
        return Err("没有可打开的本地图片文件".into());
    };

    let canon = crate::security::resolve_media_file(state.db.media_root(), rel)?;
    open_path_with_default_app(&canon)
}

#[cfg(windows)]
fn open_path_with_default_app(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    // ShellExecuteW avoids cmd.exe metacharacter injection from `cmd /C start`.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        return Err(format!("打开失败 (ShellExecute={})", result as isize));
    }
    Ok(())
}

#[cfg(not(windows))]
fn open_path_with_default_app(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("打开失败: {e}"))?;
    Ok(())
}

/// Open a whitelisted link URI via the OS handler (browser / BT client / etc.).
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !crate::security::is_openable_link(trimmed) {
        return Err("仅允许打开受支持的链接协议".into());
    }
    // ShellExecute accepts URI strings; keep the validated trimmed form (ed2k pipes etc.).
    open_path_with_default_app(std::path::Path::new(trimmed))
}

/// Remember the current foreground app as paste destination (safe no-op if FG is us).
#[tauri::command]
pub async fn capture_paste_target(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let our = window.hwnd().ok().map(|h| h.0 as isize);
        clipboard::set_our_main_hwnd(our);
        clipboard::remember_paste_target(our);
    }
    Ok(())
}

async fn focus_paste_target_on_main_thread(app: &tauri::AppHandle, hwnd: isize) -> bool {
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    mode: Option<String>,
) -> Result<(), String> {
    use tauri::Manager;

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
    struct PasteFocusUnlock<'a>(&'a tauri::AppHandle);
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

#[tauri::command]
pub async fn delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.trash_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Batch)?;
    state.db.trash_records_batch(&cap_ids(ids)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.restore_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Batch)?;
    state.db.restore_records_batch(&cap_ids(ids)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanently_delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.permanently_delete_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanently_delete_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Batch)?;
    state.db.permanently_delete_records_batch(&cap_ids(ids)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_expired(state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    state.db.cleanup_expired().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    state.db.empty_trash().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_trash_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.db.get_trash_count().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_favorite(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_favorite(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn batch_set_favorite(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    favorite: bool,
) -> Result<usize, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Batch)?;
    state
        .db
        .batch_set_favorite(&cap_ids(ids), favorite)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pin(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_record_alias(
    state: State<'_, AppState>,
    id: i64,
    alias: String,
) -> Result<String, String> {
    state
        .db
        .set_record_alias(id, &alias)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let arc = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok((*arc).clone())
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
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

    state.db.cleanup_retention(settings.retention_days).map_err(|e| e.to_string())?;
    if let Err(e) = state.db.save_settings(&settings) {
        if autostart_changed {
            if let Err(revert_err) = apply_autostart(&app, previous.auto_start) {
                warn!("Failed to revert autostart after settings save error: {}", revert_err);
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
            let _ = crate::window::apply_window_round_corners(&window, settings.panel_radius);
        }
    }
    let _ = app.emit("settings-updated", ());
    Ok(())
}

pub(crate) fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
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
    app: tauri::AppHandle,
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
            if let Some(window) = app.get_webview_window("main") {
                let our = window.hwnd().ok().map(|h| h.0 as isize);
                clipboard::set_our_main_hwnd(our);
                clipboard::remember_paste_target(our);
                let _ = window.unminimize();
                let _ = window.show();
                // Focus while tray-menu still owns foreground.
                if let Some(hwnd) = our {
                    let _ = clipboard::focus_window(hwnd);
                } else {
                    let _ = window.set_focus();
                }
                hide_tray();
                let _ = app.emit("toggle-panel", true);
                let _ = app.emit("open-settings", ());
            } else {
                hide_tray();
            }
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

/// Stream records as a JSON array directly to `path` (no full in-memory buffer).
#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, path: String) -> Result<(), String> {
    use std::fs::File;
    use std::io::{BufWriter, Write};

    let path = crate::security::validate_json_io_path(&path, true)?;
    let file = File::create(&path).map_err(|e| format!("无法创建导出文件: {e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"[\n").map_err(|e| e.to_string())?;

    let page_size = 200;
    let mut offset = 0;
    let mut first = true;
    loop {
        let batch = state
            .db
            .get_records_for_export(page_size, offset)
            .map_err(|e| e.to_string())?;
        let len = batch.len();
        for rec in &batch {
            if !first {
                w.write_all(b",\n").map_err(|e| e.to_string())?;
            }
            first = false;
            serde_json::to_writer(&mut w, rec).map_err(|e| e.to_string())?;
        }
        if len < page_size as usize {
            break;
        }
        offset += page_size;
    }

    w.write_all(b"\n]\n").map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn import_data(state: State<'_, AppState>, records: Vec<ClipboardRecord>) -> Result<i32, String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    state.db.import_records(&records, settings.max_records).map_err(|e| e.to_string())
}

/// Read a JSON backup from disk (path from native dialog) and import with sanitization.
#[tauri::command]
pub async fn import_data_from_path(state: State<'_, AppState>, path: String) -> Result<i32, String> {
    let path = crate::security::validate_json_io_path(&path, false)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("无法读取备份文件: {e}"))?;
    // Cap import size to limit memory DoS from huge malicious files.
    const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
    if text.len() > MAX_IMPORT_BYTES {
        return Err("备份文件过大（上限 64MB）".into());
    }
    let records: Vec<ClipboardRecord> =
        serde_json::from_str(&text).map_err(|e| format!("备份文件格式不正确: {e}"))?;
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    state
        .db
        .import_records(&records, settings.max_records)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_non_favorite().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<StatsData, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Stats)?;
    // Cleanup stays on the periodic background thread — stats is a hot UI poll.
    state.db.get_stats().map_err(|e| e.to_string())
}

// === Tag Commands ===

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_tags(
    state: State<'_, AppState>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
) -> Result<Vec<TagInfo>, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state
        .db
        .get_all_tags(content_type.as_deref(), favorites_only.unwrap_or(false))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_tag(state: State<'_, AppState>, name: String, color: String) -> Result<TagInfo, String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    let id = state.db.create_tag(&name, &color).map_err(|e| e.to_string())?;
    Ok(TagInfo { id, name, color, is_auto: false, count: 0 })
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state.db.delete_tag(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_tag(state: State<'_, AppState>, id: i64, name: String, color: String) -> Result<(), String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state.db.update_tag(id, &name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_tag_to_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state.db.add_tag_to_record(record_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_tag_from_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state.db.remove_tag_from_record(record_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_record_tags(
    state: State<'_, AppState>,
    record_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String> {
    require_feature(&(*state.db.get_settings().map_err(|e| e.to_string())?), FeatureId::Tags)?;
    state
        .db
        .set_record_tags(record_id, &cap_ids(tag_ids))
        .map_err(|e| e.to_string())
}

// === App Mode / Window Commands ===

#[tauri::command]
pub async fn switch_app_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    let is_window = mode == "window";
    let settings = match app.try_state::<AppState>().and_then(|s| s.db.get_settings().ok()) {
        Some(s) => (*s).clone(),
        None => {
            warn!("Failed to load settings for mode switch; using defaults");
            Settings::default()
        }
    };
    let (w, h) = crate::window::resolve_panel_size(&window, &settings, is_window);
    let (min_w, min_h, _, _) = crate::window::mode_size_bounds(is_window);
    window.set_decorations(false).map_err(|e| e.to_string())?;
    let _ = window.set_shadow(false);
    window.set_always_on_top(!is_window).map_err(|e| e.to_string())?;
    window.set_skip_taskbar(!is_window).map_err(|e| e.to_string())?;
    // Both modes resizable so remembered size can be adjusted
    window.set_resizable(true).map_err(|e| e.to_string())?;
    let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
        min_w, min_h,
    ))));
    // Cancel pending resize-save so programmatic set_size doesn't overwrite
    // the other mode's remembered size while app_mode is mid-switch.
    crate::window::SIZE_SAVE_GEN.fetch_add(1, AtomicOrdering::Relaxed);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
    // Re-apply rounded region after size change
    let _ = crate::window::apply_window_round_corners(&window, settings.panel_radius);
    info!(
        "App mode switched to: {} (size {}x{})",
        mode, w, h
    );
    Ok(())
}

#[tauri::command]
pub async fn set_window_corner_radius(app: tauri::AppHandle, radius: i32) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    crate::window::apply_window_round_corners(&window, radius)
}

/// Enable/disable native DWM frosted-glass backdrop (acrylic) on all app windows.
/// Applied by the frontend from the 毛玻璃 toggle; the same command is re-run
/// when the tray-menu window is (re)created so it follows settings too.
#[tauri::command]
pub async fn set_window_backdrop(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    for label in ["main", "tray-menu"] {
        if let Some(window) = app.get_webview_window(label) {
            crate::window::apply_window_backdrop(&window, enabled)?;
        }
    }
    Ok(())
}

// === WebDAV sync ===

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_test_connection(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature(&settings, FeatureId::Sync)?;
    crate::webdav::webdav_test_connection(&settings).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_pull(
    state: State<'_, AppState>,
) -> Result<crate::webdav::WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    crate::webdav::webdav_pull(&state.db, &mut settings).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_push(
    state: State<'_, AppState>,
) -> Result<crate::webdav::WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    crate::webdav::webdav_push(&state.db, &mut settings).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_sync(
    state: State<'_, AppState>,
) -> Result<crate::webdav::WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    crate::webdav::webdav_sync(&state.db, &mut settings).await
}
