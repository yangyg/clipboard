//! Tag CRUD and record↔tag commands.
use tauri::State;

use crate::db::nearest_palette_color;
use crate::{require_feature, AppState, FeatureId, TagInfo};

use super::{cap_ids, spawn_db};

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
    let perf_start = std::time::Instant::now();
    let db = state.db.clone();
    let tags = spawn_db(move || {
        db.get_all_tags(content_type.as_deref(), favorites_only.unwrap_or(false))
            .map_err(|e| e.to_string())
    })
    .await?;
    crate::perf::log_elapsed("get_all_tags", perf_start);
    Ok(tags)
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
    let db = state.db.clone();
    let name_for_db = name.clone();
    let color_for_db = color.clone();
    let id = spawn_db(move || {
        db.create_tag(&name_for_db, &color_for_db).map_err(|e| {
            // Surface name collisions as a stable marker so the UI can show a
            // localized "tag name already exists" message instead of the raw
            // SQLite constraint error.
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed: tags.name") {
                "TAG_NAME_EXISTS".to_string()
            } else {
                msg
            }
        })
    })
    .await?;
    Ok(TagInfo {
        id,
        name,
        color,
        is_auto: false,
        count: 0,
        synced: false,
    })
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Tags,
    )?;
    let db = state.db.clone();
    spawn_db(move || db.delete_tag(id)).await
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
    let db = state.db.clone();
    spawn_db(move || db.update_tag(id, &name, &color)).await
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
    let db = state.db.clone();
    spawn_db(move || db.add_tag_to_record(record_id, tag_id)).await
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
    let db = state.db.clone();
    spawn_db(move || db.remove_tag_from_record(record_id, tag_id)).await
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
    let tag_ids = cap_ids(tag_ids);
    let db = state.db.clone();
    spawn_db(move || db.set_record_tags(record_id, &tag_ids)).await
}
