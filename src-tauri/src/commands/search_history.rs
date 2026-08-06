//! Search-history commands (autocomplete dropdown persistence).
//! Local-only by design — these rows never participate in export/import or
//! WebDAV sync, keeping the on-device blast radius of search terms unchanged.
use tauri::State;

use crate::{AppState, SearchHistoryEntry};

#[tauri::command(rename_all = "snake_case")]
pub async fn get_search_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<SearchHistoryEntry>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 50);
    state
        .db
        .get_search_history(limit)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn record_search_history(
    state: State<'_, AppState>,
    query: String,
) -> Result<(), String> {
    state
        .db
        .record_search_history(&query)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_search_history(
    state: State<'_, AppState>,
    query: String,
) -> Result<(), String> {
    state
        .db
        .remove_search_history(&query)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn clear_search_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_search_history().map_err(|e| e.to_string())
}
