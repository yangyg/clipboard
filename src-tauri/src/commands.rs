//! All `#[tauri::command]` handlers plus autostart helpers. Extracted from
//! `lib.rs`; behaviour unchanged. Cross-module helpers are referenced via
//! fully-qualified `crate::` paths.

use std::sync::atomic::Ordering as AtomicOrdering;

use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tracing::{info, warn};

use crate::{clipboard, media};
use crate::{
    AppState, ClipboardRecord, RecordsPage, SearchResult, Settings, StatsData, TagInfo,
};

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
) -> Result<RecordsPage, String> {
    crate::maybe_run_cleanup(&state.db)?;
    let limit = limit.unwrap_or(60).max(1);
    let offset = offset.unwrap_or(0).max(0);
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
    let limit = limit.unwrap_or(60).max(1);
    let offset = offset.unwrap_or(0).max(0);
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
        )
        .map_err(|e| e.to_string())?;
    let has_more = records.len() as i32 >= limit;
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
/// Prefer this over `shell.open`: shell's allow-open only scopes http(s)/mailto/tel.
#[tauri::command]
pub async fn open_record_media(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let record = state.db.get_record(id).map_err(|e| e.to_string())?;
    let Some(r) = record else {
        return Err("记录不存在".into());
    };
    let Some(rel) = r.media_path.as_deref().filter(|s| !s.is_empty()) else {
        return Err("没有可打开的本地图片文件".into());
    };

    let abs = media::absolute(state.db.media_root(), rel);
    let root = state
        .db
        .media_root()
        .canonicalize()
        .unwrap_or_else(|_| state.db.media_root().to_path_buf());
    let canon = abs
        .canonicalize()
        .map_err(|_| format!("图片文件不存在: {}", abs.display()))?;
    if !canon.starts_with(&root) {
        return Err("路径不在媒体目录内".into());
    }
    if !canon.is_file() {
        return Err(format!("图片文件不存在: {}", canon.display()));
    }

    open_path_with_default_app(&canon)
}

#[cfg(windows)]
fn open_path_with_default_app(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // `start` requires an empty window-title arg when the path may contain spaces.
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("打开失败: {e}"))?;
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

#[tauri::command]
pub async fn paste_record(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    mode: Option<String>,
) -> Result<(), String> {
    use tauri::Manager;

    let record = state
        .db
        .take_record_for_paste(id)
        .map_err(|e| e.to_string())?;
    let Some(r) = record else {
        return Ok(());
    };

    // Paste writes the OS clipboard; suppress re-capture so CF_HTML / plain
    // round-trips don't create a duplicate row (hash includes HTML).
    state
        .monitor
        .read()
        .suppress_self_writes(std::time::Duration::from_millis(1500));

    let wrote = if r.content_type == "image" {
        if let Some(media_path) = r.media_path.as_deref() {
            let (rgba, w, h) = media::load_image_rgba(state.db.media_root(), media_path)?;
            clipboard::write_clipboard_image(&rgba, w, h)
        } else {
            return Err("Image file missing for this record".into());
        }
    } else {
        match mode.as_deref() {
            Some("plain") => clipboard::write_clipboard_plain(&r.content),
            _ => clipboard::write_clipboard_text(&r.content, r.content_html.as_deref()),
        }
    };
    if !wrote {
        return Err("Failed to set clipboard".into());
    }

    let settings = state.db.get_settings().unwrap_or_default();
    let is_floating = settings.app_mode != "window";
    let auto_close = settings.auto_close_on_paste;

    // Floating: hide so we can yield focus. Window mode stays open behind the target.
    if is_floating {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        let _ = app.emit("toggle-panel", false);
    }

    let target = clipboard::paste_target_hwnd();
    let focused = target
        .map(clipboard::focus_window)
        .unwrap_or(false);

    if focused {
        clipboard::simulate_paste();
    } else {
        warn!(
            "Paste target unavailable (hwnd={:?}); clipboard updated, skipped Ctrl+V",
            target
        );
    }

    // Floating + keep panel open: bring ourselves back after pasting into the target.
    if is_floating && !auto_close {
        if let Some(window) = app.get_webview_window("main") {
            let our = window.hwnd().ok().map(|h| h.0 as isize);
            clipboard::remember_paste_target(our);
            let _ = window.show();
            let _ = window.set_focus();
        }
        let _ = app.emit("toggle-panel", true);
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.trash_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    state.db.trash_records_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.restore_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    state.db.restore_records_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanently_delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.permanently_delete_record(id).map_err(|e| e.to_string())
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
    state
        .db
        .batch_set_favorite(&ids, favorite)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pin(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state.db.get_settings().map_err(|e| e.to_string())
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

/// Stream records as a JSON array directly to `path` (no full in-memory buffer).
#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, path: String) -> Result<(), String> {
    use std::fs::File;
    use std::io::{BufWriter, Write};

    let file = File::create(&path).map_err(|e| format!("无法创建导出文件: {e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"[\n").map_err(|e| e.to_string())?;

    let page_size = 200;
    let mut offset = 0;
    let mut first = true;
    loop {
        let batch = state
            .db
            .get_records(page_size, offset, false, None, false, None, None)
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

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_non_favorite().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<StatsData, String> {
    // Cleanup stays on capture / list paths (60s throttle) — stats is a hot UI poll.
    state.db.get_stats().map_err(|e| e.to_string())
}

// === Tag Commands ===

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_tags(
    state: State<'_, AppState>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
) -> Result<Vec<TagInfo>, String> {
    state
        .db
        .get_all_tags(content_type.as_deref(), favorites_only.unwrap_or(false))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_tag(state: State<'_, AppState>, name: String, color: String) -> Result<TagInfo, String> {
    let id = state.db.create_tag(&name, &color).map_err(|e| e.to_string())?;
    Ok(TagInfo { id, name, color, is_auto: false, count: 0 })
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.delete_tag(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_tag(state: State<'_, AppState>, id: i64, name: String, color: String) -> Result<(), String> {
    state.db.update_tag(id, &name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_tag_to_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    state.db.add_tag_to_record(record_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_tag_from_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    state.db.remove_tag_from_record(record_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_record_tags(
    state: State<'_, AppState>,
    record_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String> {
    state
        .db
        .set_record_tags(record_id, &tag_ids)
        .map_err(|e| e.to_string())
}

// === App Mode / Window Commands ===

#[tauri::command]
pub async fn switch_app_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    let is_window = mode == "window";
    let settings = app
        .try_state::<AppState>()
        .and_then(|s| s.db.get_settings().ok())
        .unwrap_or_default();
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
