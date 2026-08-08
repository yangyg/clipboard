//! Tag CRUD and record↔tag commands.
use tauri::State;

use crate::db::nearest_palette_color;
use crate::{require_feature, AppState, FeatureId, TagInfo};

use super::cap_ids;

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_tags(
    state: State<'_, AppState>,
    content_type: Option<String>,
    favorites_only: Option<bool>,
) -> Result<Vec<TagInfo>, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    state
        .db
        .get_all_tags(content_type.as_deref(), favorites_only.unwrap_or(false))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_tag(
    state: State<'_, AppState>,
    name: String,
    color: String,
) -> Result<TagInfo, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    // Snap arbitrary input onto the fixed 12-color wheel at the IPC boundary so
    // the DB never stores a string that could be injected into CSS color-mix.
    let color = nearest_palette_color(&color).to_string();
    let id = state
        .db
        .create_tag(&name, &color)
        .map_err(|e| e.to_string())?;
    Ok(TagInfo {
        id,
        name,
        color,
        is_auto: false,
        count: 0,
    })
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    state.db.delete_tag(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_tag(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    color: String,
) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    let color = nearest_palette_color(&color).to_string();
    state
        .db
        .update_tag(id, &name, &color)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn add_tag_to_record(
    state: State<'_, AppState>,
    record_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    state
        .db
        .add_tag_to_record(record_id, tag_id)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_tag_from_record(
    state: State<'_, AppState>,
    record_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    state
        .db
        .remove_tag_from_record(record_id, tag_id)
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_record_tags(
    state: State<'_, AppState>,
    record_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    state
        .db
        .set_record_tags(record_id, &cap_ids(tag_ids))
        .map_err(|e| e.to_string())
}
