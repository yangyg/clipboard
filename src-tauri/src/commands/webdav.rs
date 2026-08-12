//! WebDAV sync commands (test connection / pull / push / sync / history).
//! Each pull/push/sync run is recorded into the local `sync_history` log
//! (success carries the structured counters; failures carry the error text).
use tauri::State;

use crate::webdav::WebDavSyncResult;
use crate::{require_feature, AppState, FeatureId, SyncHistoryEntry};

use super::spawn_db;

fn log_sync_ok(db: &crate::db::ClipboardDb, action: &str, r: &WebDavSyncResult) {
    let _ = db.insert_sync_history(
        action,
        true,
        r.pulled,
        r.pushed,
        r.merged,
        r.tags_pulled,
        r.tags_pushed,
        r.media_downloaded,
        r.media_uploaded,
        r.media_skipped,
        None,
    );
}

fn log_sync_err(db: &crate::db::ClipboardDb, action: &str, error: &str) {
    let _ = db.insert_sync_history(action, false, 0, 0, 0, 0, 0, 0, 0, 0, Some(error));
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_test_connection(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature(&settings, FeatureId::Sync)?;
    crate::webdav::webdav_test_connection(&settings).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_pull(state: State<'_, AppState>) -> Result<WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    let result = crate::webdav::webdav_pull(&state.db, &mut settings).await;
    match &result {
        Ok(r) => log_sync_ok(&state.db, "pull", r),
        Err(e) => log_sync_err(&state.db, "pull", e),
    }
    result
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_push(state: State<'_, AppState>) -> Result<WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    let result = crate::webdav::webdav_push(&state.db, &mut settings).await;
    match &result {
        Ok(r) => log_sync_ok(&state.db, "push", r),
        Err(e) => log_sync_err(&state.db, "push", e),
    }
    result
}

#[tauri::command(rename_all = "snake_case")]
pub async fn webdav_sync(state: State<'_, AppState>) -> Result<WebDavSyncResult, String> {
    let mut settings = (*state.db.get_settings().map_err(|e| e.to_string())?).clone();
    require_feature(&settings, FeatureId::Sync)?;
    let result = crate::webdav::webdav_sync(&state.db, &mut settings).await;
    match &result {
        Ok(r) => log_sync_ok(&state.db, "sync", r),
        // A failed full sync (e.g. push failed after a successful pull) is logged
        // as a single failure with the error text; the partial pull counters are
        // not persisted separately.
        Err(e) => log_sync_err(&state.db, "sync", e),
    }
    result
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_sync_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<SyncHistoryEntry>, String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature(&settings, FeatureId::Sync)?;
    let db = state.db.clone();
    spawn_db(move || db.get_sync_history(limit.unwrap_or(20))).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn clear_sync_history(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature(&settings, FeatureId::Sync)?;
    let db = state.db.clone();
    spawn_db(move || db.clear_sync_history()).await
}
