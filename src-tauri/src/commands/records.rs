//! Record CRUD / search / trash / favorite / pin / alias commands.
use tauri::State;

use crate::security;
use crate::{require_feature, AppState, ClipboardRecord, FeatureId, RecordsPage, SearchResult};

use super::{cap_ids, settings_features, MAX_PAGE_SIZE};

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
    before_pinned: Option<i32>,
    before_updated_at: Option<String>,
    before_id: Option<i64>,
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
            before_pinned,
            before_updated_at.as_deref(),
            before_id,
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
pub async fn get_record(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<ClipboardRecord>, String> {
    state.db.get_record(id).map_err(|e| e.to_string())
}

/// Batch full-row read in a single IN query — replaces N concurrent
/// `get_record` IPC round-trips (batch copy reads full content for N rows).
#[tauri::command(rename_all = "snake_case")]
pub async fn get_records_by_ids(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<Vec<ClipboardRecord>, String> {
    let ids = cap_ids(ids);
    state.db.get_records_by_ids(&ids).map_err(|e| e.to_string())
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

    let canon = security::resolve_media_file(state.db.media_root(), rel)?;
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
    if !security::is_openable_link(trimmed) {
        return Err("仅允许打开受支持的链接协议".into());
    }
    // ShellExecute accepts URI strings; keep the validated trimmed form (ed2k pipes etc.).
    open_path_with_default_app(std::path::Path::new(trimmed))
}

#[tauri::command]
pub async fn delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.trash_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_records_batch(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Batch,
    )?;
    state
        .db
        .trash_records_batch(&cap_ids(ids))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.db.restore_record(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_records_batch(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Batch,
    )?;
    state
        .db
        .restore_records_batch(&cap_ids(ids))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanently_delete_record(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state
        .db
        .permanently_delete_record(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn permanently_delete_records_batch(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Batch,
    )?;
    state
        .db
        .permanently_delete_records_batch(&cap_ids(ids))
        .map_err(|e| e.to_string())
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
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Batch,
    )?;
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
