mod db;
mod clipboard;
mod media;

use db::{ClipboardDb, ContentType, ImageMeta};
use clipboard::{ClipboardMonitor, ClipboardEvent};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, LazyLock};
use parking_lot::RwLock;
use regex::Regex;
use tauri::{Emitter, Manager, State};
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

// === App State ===
pub struct AppState {
    pub db: Arc<ClipboardDb>,
    pub monitor: Arc<RwLock<ClipboardMonitor>>,
    pub capture_paused: Arc<RwLock<bool>>,
}

// === Tauri Record Type (must match src/types.ts ClipboardRecord) ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    #[serde(rename = "source_app")]
    pub source_app: String,
    #[serde(rename = "source_window")]
    pub source_window: String,
    pub hash: String,
    #[serde(rename = "copy_count")]
    pub copy_count: i32,
    #[serde(rename = "is_favorite")]
    pub is_favorite: bool,
    #[serde(rename = "is_pinned")]
    pub is_pinned: bool,
    #[serde(rename = "is_sensitive")]
    pub is_sensitive: bool,
    #[serde(rename = "is_trashed")]
    pub is_trashed: bool,
    #[serde(rename = "auto_expire_at")]
    pub auto_expire_at: Option<String>,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    pub tags: Vec<String>,
    /// HTML clipboard fragment when format was captured (Word, browser, etc.)
    #[serde(rename = "content_html")]
    pub content_html: Option<String>,
    /// Relative path under app data dir, e.g. media/{hash}.png
    #[serde(rename = "media_path")]
    pub media_path: Option<String>,
    #[serde(rename = "thumb_path")]
    pub thumb_path: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Absolute filesystem paths for frontend convertFileSrc
    #[serde(rename = "media_abs")]
    pub media_abs: Option<String>,
    #[serde(rename = "thumb_abs")]
    pub thumb_abs: Option<String>,
    /// Full content character length (list rows may truncate `content`)
    #[serde(rename = "content_len")]
    pub content_len: Option<i32>,
}

// === Settings (must match src/types.ts Settings) ===
#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "global_shortcut")]
    pub global_shortcut: String,
    #[serde(rename = "max_records")]
    pub max_records: i32,
    #[serde(rename = "retention_days")]
    pub retention_days: i32,
    pub theme: String,
    #[serde(rename = "panel_opacity")]
    pub panel_opacity: i32,
    #[serde(rename = "panel_radius")]
    pub panel_radius: i32,
    #[serde(rename = "enable_blur")]
    pub enable_blur: bool,
    #[serde(rename = "enable_animation")]
    pub enable_animation: bool,
    #[serde(rename = "font_size")]
    pub font_size: i32,
    #[serde(rename = "app_mode")]
    pub app_mode: String,
    #[serde(rename = "default_paste_mode")]
    pub default_paste_mode: String,
    #[serde(rename = "auto_close_on_paste")]
    pub auto_close_on_paste: bool,
    #[serde(rename = "enable_sensitive_detection")]
    pub enable_sensitive_detection: bool,
    #[serde(rename = "sensitive_auto_expire_seconds")]
    pub sensitive_auto_expire_seconds: i32,
    #[serde(rename = "data_path")]
    pub data_path: String,
    #[serde(rename = "auto_start")]
    pub auto_start: bool,
    #[serde(rename = "minimize_to_tray")]
    pub minimize_to_tray: bool,
    #[serde(rename = "ignored_apps")]
    pub ignored_apps: Vec<String>,
    /// Remembered logical size (0 = use adaptive default). Per app mode.
    #[serde(default, rename = "floating_width")]
    pub floating_width: i32,
    #[serde(default, rename = "floating_height")]
    pub floating_height: i32,
    #[serde(default, rename = "window_width")]
    pub window_width: i32,
    #[serde(default, rename = "window_height")]
    pub window_height: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            global_shortcut: "Ctrl+Shift+V".to_string(),
            max_records: 1000,
            retention_days: 30,
            theme: "dark".to_string(),
            panel_opacity: 94,
            panel_radius: 20,
            enable_blur: true,
            enable_animation: true,
            font_size: 16,
            app_mode: "floating".to_string(),
            default_paste_mode: "original".to_string(),
            auto_close_on_paste: true,
            enable_sensitive_detection: true,
            sensitive_auto_expire_seconds: 600,
            data_path: "".to_string(),
            auto_start: false,
            minimize_to_tray: true,
            ignored_apps: vec![
                "1Password.exe".to_string(),
                "ICBCNetBank.exe".to_string(),
            ],
            floating_width: 0,
            floating_height: 0,
            window_width: 0,
            window_height: 0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub records: Vec<ClipboardRecord>,
    pub total: usize,
    pub query: String,
    #[serde(rename = "elapsed_ms")]
    pub elapsed_ms: u64,
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct RecordsPage {
    pub records: Vec<ClipboardRecord>,
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub is_auto: bool,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsData {
    #[serde(rename = "total_records")]
    pub total_records: i64,
    #[serde(rename = "total_copies")]
    pub total_copies: i64,
    #[serde(rename = "favorites_count")]
    pub favorites_count: i64,
    #[serde(rename = "pinned_count")]
    pub pinned_count: i64,
    #[serde(rename = "sensitive_count")]
    pub sensitive_count: i64,
    #[serde(rename = "storage_bytes")]
    pub storage_bytes: i64,
    #[serde(rename = "type_distribution")]
    pub type_distribution: std::collections::HashMap<String, i64>,
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "snake_case")]
async fn get_records(
    state: State<'_, AppState>,
    limit: Option<i32>,
    offset: Option<i32>,
    trashed: Option<bool>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
    tag: Option<String>,
) -> Result<RecordsPage, String> {
    maybe_run_cleanup(&state.db)?;
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
        )
        .map_err(|e| e.to_string())?;
    let has_more = records.len() as i32 >= limit;
    Ok(RecordsPage { records, has_more })
}

#[tauri::command(rename_all = "snake_case")]
async fn search_records(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
    offset: Option<i32>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
    tag: Option<String>,
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
async fn get_record(state: State<'_, AppState>, id: i64) -> Result<Option<ClipboardRecord>, String> {
    state.db.get_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn paste_record(
    state: State<'_, AppState>,
    id: i64,
    mode: Option<String>,
) -> Result<(), String> {
    let record = state.db.get_record(id).map_err(|e| e.to_string())?;
    if let Some(r) = record {
        let _ = state.db.increment_copy_count(id);
        if r.content_type == "image" {
            if let Some(media_path) = r.media_path.as_deref() {
                let (rgba, w, h) = media::load_image_rgba(state.db.media_root(), media_path)?;
                clipboard::paste_image(&rgba, w, h);
            } else {
                return Err("Image file missing for this record".into());
            }
        } else {
            match mode.as_deref() {
                Some("plain") => clipboard::paste_plain_text(&r.content),
                _ => clipboard::paste_text(&r.content, r.content_html.as_deref()),
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.trash_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    state.db.trash_records_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
async fn restore_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.restore_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn restore_records_batch(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    state.db.restore_records_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
async fn permanently_delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.permanently_delete_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn cleanup_expired(state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    state.db.cleanup_expired().map_err(|e| e.to_string())
}

#[tauri::command]
async fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    state.db.empty_trash().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_trash_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.db.get_trash_count().map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_favorite(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_favorite(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn batch_set_favorite(
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
async fn toggle_pin(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state.db.get_settings().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(
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
        if let Err(e) = apply_global_shortcut(&app, &settings.global_shortcut) {
            warn!("Failed to apply new global shortcut: {}", e);
            // Persist succeeded; surface so UI can reload / warn
            return Err(e);
        }
    }

    if settings.panel_radius != previous.panel_radius {
        if let Some(window) = app.get_webview_window("main") {
            let _ = apply_window_round_corners(&window, settings.panel_radius);
        }
    }
    Ok(())
}

fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
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
async fn set_capture_paused(state: State<'_, AppState>, paused: bool) -> Result<(), String> {
    *state.capture_paused.write() = paused;
    info!("Capture paused: {}", paused);
    Ok(())
}

#[tauri::command]
async fn export_data(state: State<'_, AppState>) -> Result<String, String> {
    let records = state
        .db
        .get_records(10000, 0, false, None, false, None)
        .map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&records).map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_data(state: State<'_, AppState>, records: Vec<ClipboardRecord>) -> Result<i32, String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    state.db.import_records(&records, settings.max_records).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_non_favorite().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_stats(state: State<'_, AppState>) -> Result<StatsData, String> {
    maybe_run_cleanup(&state.db)?;
    state.db.get_stats().map_err(|e| e.to_string())
}

// === Tag Commands ===

#[tauri::command(rename_all = "snake_case")]
async fn get_all_tags(
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
async fn create_tag(state: State<'_, AppState>, name: String, color: String) -> Result<TagInfo, String> {
    let id = state.db.create_tag(&name, &color).map_err(|e| e.to_string())?;
    Ok(TagInfo { id, name, color, is_auto: false, count: 0 })
}

#[tauri::command]
async fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.delete_tag(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_tag(state: State<'_, AppState>, id: i64, name: String, color: String) -> Result<(), String> {
    state.db.update_tag(id, &name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_tag_to_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    state.db.add_tag_to_record(record_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_tag_from_record(state: State<'_, AppState>, record_id: i64, tag_id: i64) -> Result<(), String> {
    state.db.remove_tag_from_record(record_id, tag_id).map_err(|e| e.to_string())
}

// === App Mode ===

fn monitor_work_area_logical(window: &tauri::WebviewWindow) -> (f64, f64) {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .map(|m| {
            let scale = m.scale_factor().max(0.5);
            let area = m.work_area();
            let w = (area.size.width as f64 / scale).max(320.0);
            let h = (area.size.height as f64 / scale).max(320.0);
            (w, h)
        })
        .unwrap_or((1920.0, 1080.0))
}

/// Logical panel size from the current (or primary) monitor work area.
/// Floating ≈ 40%×65% of screen; window ≈ 55%×72%; clamped so small laptops
/// stay usable and large 4K screens don't open a huge panel.
fn adaptive_panel_size(window: &tauri::WebviewWindow, is_window_mode: bool) -> (f64, f64) {
    let (frac_w, frac_h, min_w, min_h, max_w, max_h) = if is_window_mode {
        (0.55, 0.72, 720.0, 520.0, 1280.0, 900.0)
    } else {
        (0.40, 0.65, 480.0, 480.0, 800.0, 780.0)
    };

    let (screen_w, screen_h) = monitor_work_area_logical(window);

    let w = (screen_w * frac_w)
        .clamp(min_w, max_w)
        .min((screen_w - 32.0).max(min_w));
    let h = (screen_h * frac_h)
        .clamp(min_h, max_h)
        .min((screen_h - 32.0).max(min_h));
    (w.round(), h.round())
}

fn mode_size_bounds(is_window_mode: bool) -> (f64, f64, f64, f64) {
    if is_window_mode {
        (480.0, 400.0, 1600.0, 1200.0)
    } else {
        (400.0, 400.0, 1000.0, 900.0)
    }
}

/// Prefer remembered size when valid; otherwise adaptive. Always clamp to work area.
fn resolve_panel_size(
    window: &tauri::WebviewWindow,
    settings: &Settings,
    is_window_mode: bool,
) -> (f64, f64) {
    let (saved_w, saved_h) = if is_window_mode {
        (settings.window_width, settings.window_height)
    } else {
        (settings.floating_width, settings.floating_height)
    };
    let (min_w, min_h, max_w, max_h) = mode_size_bounds(is_window_mode);
    let (screen_w, screen_h) = monitor_work_area_logical(window);

    if saved_w >= min_w as i32 && saved_h >= min_h as i32 {
        let w = (saved_w as f64)
            .clamp(min_w, max_w)
            .min((screen_w - 32.0).max(min_w));
        let h = (saved_h as f64)
            .clamp(min_h, max_h)
            .min((screen_h - 32.0).max(min_h));
        return (w.round(), h.round());
    }
    adaptive_panel_size(window, is_window_mode)
}

fn persist_current_window_size(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    // Don't remember maximized geometry as the restored size
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
    let w = ((size.width as f64) / scale).round() as i32;
    let h = ((size.height as f64) / scale).round() as i32;
    if w < 200 || h < 200 {
        return;
    }

    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(mut settings) = state.db.get_settings() else {
        return;
    };
    // Prefer live window chrome over DB app_mode (mode switch may not be saved yet).
    let is_window = !window.is_always_on_top().unwrap_or(settings.app_mode == "window");
    let (min_w, min_h, _, _) = mode_size_bounds(is_window);
    if (w as f64) < min_w || (h as f64) < min_h {
        return;
    }

    if is_window {
        if settings.window_width == w && settings.window_height == h {
            return;
        }
        settings.window_width = w;
        settings.window_height = h;
    } else {
        if settings.floating_width == w && settings.floating_height == h {
            return;
        }
        settings.floating_width = w;
        settings.floating_height = h;
    }
    if let Err(e) = state.db.save_settings(&settings) {
        warn!("Failed to persist window size: {}", e);
    } else {
        let mode = if is_window { "window" } else { "floating" };
        info!("Remembered {} size {}x{}", mode, w, h);
    }
}

/// Debounce resize → settings write (Resized fires continuously while dragging).
static SIZE_SAVE_GEN: AtomicU64 = AtomicU64::new(0);

fn schedule_persist_window_size(app: tauri::AppHandle) {
    let gen = SIZE_SAVE_GEN.fetch_add(1, AtomicOrdering::Relaxed) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if SIZE_SAVE_GEN.load(AtomicOrdering::Relaxed) != gen {
            return;
        }
        if let Some(window) = app.get_webview_window("main") {
            persist_current_window_size(&app, &window);
        }
    });
}

#[tauri::command]
async fn switch_app_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    let is_window = mode == "window";
    let settings = app
        .try_state::<AppState>()
        .and_then(|s| s.db.get_settings().ok())
        .unwrap_or_default();
    let (w, h) = resolve_panel_size(&window, &settings, is_window);
    let (min_w, min_h, _, _) = mode_size_bounds(is_window);
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
    SIZE_SAVE_GEN.fetch_add(1, AtomicOrdering::Relaxed);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
    // Re-apply rounded region after size change
    let _ = apply_window_round_corners(&window, settings.panel_radius);
    info!(
        "App mode switched to: {} (size {}x{})",
        mode, w, h
    );
    Ok(())
}

/// Clip the HWND to a rounded rect so corners are not rectangular (and not black).
/// `radius` is logical CSS px from 面板外观 → 圆角大小; scaled to physical pixels.
#[tauri::command]
async fn set_window_corner_radius(app: tauri::AppHandle, radius: i32) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("window not found")?;
    apply_window_round_corners(&window, radius)
}

fn apply_window_round_corners(window: &tauri::WebviewWindow, radius_logical: i32) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};

        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let w = size.width as i32;
        let h = size.height as i32;
        let scale = window.scale_factor().unwrap_or(1.0);
        let radius = ((radius_logical.max(0) as f64) * scale).round() as i32;

        // Clear region → rectangular window
        if radius <= 0 {
            let ok = unsafe { SetWindowRgn(hwnd.0 as _, std::ptr::null_mut(), 1) };
            if ok == 0 {
                return Err("SetWindowRgn(null) failed".into());
            }
            return Ok(());
        }

        // Ellipse width/height = 2 * corner radius (Win32 convention)
        let ellipse = (radius * 2).max(1);
        // +1 on bottom-right is required by CreateRoundRectRgn (exclusive edge)
        let hrgn = unsafe { CreateRoundRectRgn(0, 0, w + 1, h + 1, ellipse, ellipse) };
        if hrgn.is_null() {
            return Err("CreateRoundRectRgn failed".into());
        }
        let ok = unsafe { SetWindowRgn(hwnd.0 as _, hrgn, 1) };
        if ok == 0 {
            unsafe {
                DeleteObject(hrgn);
            }
            return Err("SetWindowRgn failed".into());
        }
        // Ownership of hrgn transferred to the system on success
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, radius_logical);
        Ok(())
    }
}

// ============================================================
// Main Application Setup
// ============================================================

fn setup_logging(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "clipvault.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    Box::leak(Box::new(_guard));
}

pub fn run() {
    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ClipVault");
    std::fs::create_dir_all(&app_data_dir).ok();

    setup_logging(&app_data_dir);
    info!("ClipVault starting up...");

    let db_path = app_data_dir.join("clipvault.db");
    media::ensure_dirs(&app_data_dir).ok();
    let db = match ClipboardDb::new(&db_path, app_data_dir.clone()) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };
    info!("Database initialized at {:?}", db_path);

    let monitor = Arc::new(RwLock::new(ClipboardMonitor::new()));
    let capture_paused = Arc::new(RwLock::new(false));

    let app_state = AppState {
        db: db.clone(),
        monitor: monitor.clone(),
        capture_paused: capture_paused.clone(),
    };

    let db_for_setup = db.clone();
    let monitor_for_setup = monitor.clone();
    let capture_paused_for_setup = capture_paused.clone();

    tauri::Builder::default()
        // Must be registered first so a second process exits before grabbing OS resources
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            info!("Second instance detected; focusing existing window");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = app.emit("toggle-panel", true);
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None::<Vec<&'static str>>))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_records,
            search_records,
            get_record,
            paste_record,
            delete_record,
            delete_records_batch,
            restore_record,
            restore_records_batch,
            permanently_delete_record,
            empty_trash,
            cleanup_expired,
            get_trash_count,
            toggle_favorite,
            batch_set_favorite,
            toggle_pin,
            get_settings,
            save_settings,
            set_capture_paused,
            export_data,
            import_data,
            clear_history,
            get_stats,
            switch_app_mode,
            set_window_corner_radius,
            get_all_tags,
            create_tag,
            delete_tag,
            update_tag,
            add_tag_to_record,
            remove_tag_from_record,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let db = db_for_setup.clone();
            let monitor = monitor_for_setup.clone();
            let capture_paused_menu = capture_paused_for_setup.clone();
            let capture_paused_thread = capture_paused_for_setup.clone();

            // Sync OS autostart with persisted setting; skip if settings cannot be loaded
            match db.get_settings() {
                Ok(startup_settings) => {
                    if let Err(e) = apply_autostart(&app_handle, startup_settings.auto_start) {
                        warn!("Startup autostart sync failed: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to load settings for autostart sync: {}", e);
                }
            }

            let shortcut = db
                .get_settings()
                .map(|s| s.global_shortcut)
                .unwrap_or_else(|_| "Ctrl+Shift+V".into());
            if let Err(e) = apply_global_shortcut(app.handle(), &shortcut) {
                warn!("Failed to register global shortcut {}: {}", shortcut, e);
                let shortcut_label = shortcut.clone();
                app.dialog()
                    .message(format!(
                        "全局快捷键 {shortcut_label} 已被其他程序占用，无法注册。\n\n\
                         请关闭占用该快捷键的应用后重新启动剪贴板管理，或在设置中更换快捷键。\
                         若本机已有另一个剪贴板管理在运行，请先退出那个实例。"
                    ))
                    .title("剪贴板管理")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }

            // Setup system tray
            let show_item = MenuItemBuilder::with_id("show", "📋 打开面板").build(app)?;
            let pause_item = MenuItemBuilder::with_id("pause", "⏸ 暂停捕获").build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "⚙ 设置").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "❌ 退出").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&pause_item)
                .separator()
                .item(&settings_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("剪贴板管理")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                window.show().ok();
                                window.set_focus().ok();
                                app.emit("toggle-panel", true).ok();
                                info!("Tray menu: show panel");
                            }
                        }
                        "pause" => {
                            let next = !*capture_paused_menu.read();
                            *capture_paused_menu.write() = next;
                            app.emit("capture-paused", next).ok();
                            info!("Tray menu: capture paused = {}", next);
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                window.show().ok();
                                window.set_focus().ok();
                                app.emit("open-settings", ()).ok();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                window.hide().ok();
                                app.emit("toggle-panel", false).ok();
                            } else {
                                window.show().ok();
                                window.set_focus().ok();
                                app.emit("toggle-panel", true).ok();
                            }
                        }
                    }
                })
                .build(app)?;

            // Clip main window to rounded corners (avoids rectangular / black corners on Windows)
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let radius = db
                    .get_settings()
                    .map(|s| s.panel_radius)
                    .unwrap_or(20);
                if let Err(e) = apply_window_round_corners(&window, radius) {
                    warn!("Failed to apply window round corners: {}", e);
                }
            }

            // Start clipboard monitoring
            let app_handle_clone = app_handle.clone();
            let media_root = db.media_root().to_path_buf();
            std::thread::spawn(move || {
                monitor.write().start(move |event| {
                    if *capture_paused_thread.read() {
                        return;
                    }

                    // Capture foreground window info
                    let (source_window, source_app) = clipboard::get_foreground_window_info();

                    match event {
                        ClipboardEvent::Text(captured) => {
                            let settings = db.get_settings().unwrap_or_default();
                            if is_ignored_app(&source_app, &settings.ignored_apps) {
                                return;
                            }

                            let content_type = detect_content_type(&captured.text);
                            let is_sensitive =
                                settings.enable_sensitive_detection && detect_sensitive(&captured.text);
                            // Include HTML in hash so same plain text with different format is distinct
                            let hash = sha256_hash(&captured.fingerprint());

                            if let Ok(ids) = maybe_run_cleanup(&db) {
                                if !ids.is_empty() {
                                    app_handle_clone.emit("records-expired", &ids).ok();
                                }
                            }
                            match db.insert_record(
                                &captured.text,
                                &content_type,
                                &hash,
                                is_sensitive,
                                settings.max_records,
                                settings.sensitive_auto_expire_seconds,
                                &source_app,
                                &source_window,
                                None,
                                captured.html.as_deref(),
                            ) {
                                Ok(id) => {
                                    info!(
                                        "New clipboard record: id={}, type={}, formatted={}",
                                        id,
                                        content_type,
                                        captured.html.is_some()
                                    );
                                    if let Ok(record) = db.get_record(id) {
                                        if let Some(r) = record {
                                            app_handle_clone
                                                .emit("clipboard-changed", list_ipc_payload(r))
                                                .ok();
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to insert text record: {}", e);
                                }
                            }
                        }
                        ClipboardEvent::Image(captured) => {
                            let settings = db.get_settings().unwrap_or_default();
                            if is_ignored_app(&source_app, &settings.ignored_apps) {
                                return;
                            }

                            if let Ok(ids) = maybe_run_cleanup(&db) {
                                if !ids.is_empty() {
                                    app_handle_clone.emit("records-expired", &ids).ok();
                                }
                            }

                            match media::store_clipboard_image(
                                &media_root,
                                captured.rgba,
                                captured.width,
                                captured.height,
                                &captured.hash,
                            ) {
                                Ok(stored) => {
                                    let image_meta = ImageMeta {
                                        media_path: stored.media_path,
                                        thumb_path: stored.thumb_path,
                                        width: stored.width as i32,
                                        height: stored.height as i32,
                                    };
                                    // content holds a short label for search/list; binary lives on disk
                                    let label = format!(
                                        "[image {}x{}]",
                                        stored.width, stored.height
                                    );
                                    match db.insert_record(
                                        &label,
                                        &ContentType::Image,
                                        &captured.hash,
                                        false,
                                        settings.max_records,
                                        settings.sensitive_auto_expire_seconds,
                                        &source_app,
                                        &source_window,
                                        Some(&image_meta),
                                        None,
                                    ) {
                                        Ok(id) => {
                                            info!("New clipboard record: id={}, type=image", id);
                                            if let Ok(record) = db.get_record(id) {
                                                if let Some(r) = record {
                                                    app_handle_clone
                                                        .emit("clipboard-changed", list_ipc_payload(r))
                                                        .ok();
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to insert image record: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to store clipboard image: {}", e);
                                }
                            }
                        }
                    }
                });
            });

            info!("ClipVault setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    window.hide().ok();
                    api.prevent_close();
                }
                tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    if window.label() != "main" {
                        return;
                    }
                    let app = window.app_handle().clone();
                    let radius = app
                        .try_state::<AppState>()
                        .and_then(|s| s.db.get_settings().ok())
                        .map(|s| s.panel_radius)
                        .unwrap_or(20);
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = apply_window_round_corners(&w, radius);
                    }
                    // Remember user-adjusted size (debounced); skip maximized.
                    if matches!(event, tauri::WindowEvent::Resized(_)) {
                        schedule_persist_window_size(app);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================
// Pre-compiled Regex Patterns
// ============================================================

static CODE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"^\s*(fn|function|def|class|struct|impl|pub|const|let|var|import|from|require|module)\s"#,
        r#"^\s*(if|for|while|switch|match)\s*[({]"#,
        r#"^\s*(const|let|var)\s+\w+\s*[=:>]"#,
        r#"^\s*#[a-z]+\s"#,
        r#"\{\s*["']\w+["']\s*:"#,
        r#"</?[a-z][a-z0-9]*"#,
        r#"^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)\s"#,
        r#"^\s*---\s*$"#,
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static VERIFICATION_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{4,8}").unwrap());
static API_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap());
static BANK_CARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{16,19}").unwrap());

// ============================================================
// Helper Functions
// ============================================================

static LAST_CLEANUP_UNIX: AtomicU64 = AtomicU64::new(0);
const CLEANUP_INTERVAL_SECS: u64 = 60;

fn maybe_run_cleanup(db: &ClipboardDb) -> Result<Vec<i64>, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = LAST_CLEANUP_UNIX.load(AtomicOrdering::Relaxed);
    if now.saturating_sub(last) < CLEANUP_INTERVAL_SECS {
        return Ok(Vec::new());
    }
    LAST_CLEANUP_UNIX.store(now, AtomicOrdering::Relaxed);
    let expired = db.cleanup_expired().map_err(|e| e.to_string())?;
    if let Ok(settings) = db.get_settings() {
        db.cleanup_retention(settings.retention_days)
            .map_err(|e| e.to_string())?;
    }
    Ok(expired)
}

fn is_ignored_app(source_app: &str, ignored: &[String]) -> bool {
    if source_app.is_empty() || ignored.is_empty() {
        return false;
    }
    let app_lower = source_app.to_lowercase();
    ignored.iter().any(|pat| {
        let p = pat.trim().to_lowercase();
        !p.is_empty() && (app_lower == p || app_lower.ends_with(&p) || app_lower.contains(&p))
    })
}

fn apply_global_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
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
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        window.hide().ok();
                        app.emit("toggle-panel", false).ok();
                    } else {
                        window.show().ok();
                        window.set_focus().ok();
                        app.emit("toggle-panel", true).ok();
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    info!("Registered global shortcut: {}", shortcut);
    Ok(())
}

fn detect_content_type(content: &str) -> ContentType {
    let trimmed = content.trim();

    // URL detection
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("ftp://") {
        return ContentType::Link;
    }

    // File path heuristic only — avoid Path::exists() disk IO on the monitor thread
    if trimmed.len() < 2048
        && ((trimmed.contains(":\\") && trimmed.contains('.'))
            || (trimmed.starts_with('/') && trimmed.contains('.')))
    {
        return ContentType::File;
    }

    // Code detection (pre-compiled patterns)
    for re in CODE_PATTERNS.iter() {
        if re.is_match(trimmed) {
            return ContentType::Code;
        }
    }

    // JSON detection
    if (trimmed.starts_with("{") && trimmed.ends_with("}")) || (trimmed.starts_with("[") && trimmed.ends_with("]")) {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return ContentType::Code;
        }
    }

    ContentType::Text
}

/// Strip HTML and truncate content for clipboard-changed IPC (list stays light).
fn list_ipc_payload(mut r: ClipboardRecord) -> ClipboardRecord {
    r.content_html = None;
    let full_len = r
        .content_len
        .unwrap_or_else(|| r.content.chars().count() as i32);
    r.content_len = Some(full_len);
    const MAX: usize = 400;
    if r.content.chars().count() > MAX {
        r.content = r.content.chars().take(MAX).collect();
    }
    r
}

fn detect_sensitive(content: &str) -> bool {
    let trimmed = content.trim();

    // Password fields
    if trimmed.to_lowercase().contains("password") || trimmed.to_lowercase().contains("passwd") || trimmed.to_lowercase().contains("pwd") {
        return true;
    }

    // Verification codes (4-8 digit codes with "验证码" or similar)
    if VERIFICATION_CODE_RE.is_match(trimmed)
        && (trimmed.contains("验证码") || trimmed.contains("code") || trimmed.contains("Code")) {
        return true;
    }

    // API keys (sk-...)
    if API_KEY_RE.is_match(trimmed) {
        return true;
    }

    // Bank card numbers (16-19 digits)
    if BANK_CARD_RE.is_match(trimmed) && trimmed.len() <= 25 {
        return true;
    }

    false
}

fn sha256_hash(content: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}
