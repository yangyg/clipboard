//! WebDAV sync commands (test connection / pull / push / sync).
use tauri::State;

use crate::{AppState, FeatureId, require_feature};

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
