//! AI commands (feature-gated): connection test + on-demand record enrichment.
use tauri::State;

use crate::ai::{apply_on_demand, prepare_on_demand, AiClient, AiEnrichMode, AiEnrichOutcome};
use crate::{AppState, FeatureId};

use super::{require_feature_state, spawn_db};

#[tauri::command(rename_all = "snake_case")]
pub async fn ai_test_connection(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature_state(&state, FeatureId::Ai)?;
    if !settings.enable_ai {
        return Err("请先在 AI 设置中开启 AI 功能".into());
    }
    let client = AiClient::new(
        &settings.ai_base_url,
        &settings.ai_api_key,
        &settings.ai_model,
    )?;
    client.test_connection().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ai_enrich_record(
    state: State<'_, AppState>,
    id: i64,
    mode: String,
) -> Result<AiEnrichOutcome, String> {
    let mode = AiEnrichMode::parse(&mode)?;
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let record = spawn_db(move || db.get_record(id))
        .await?
        .ok_or_else(|| "记录不存在".to_string())?;
    let (content, config) = prepare_on_demand(&settings, &record, mode)?;
    let client = AiClient::new(&config.base_url, &config.api_key, &config.model)?;
    let result = client.chat_json(&content, &config.language).await?;
    let db = state.db.clone();
    spawn_db(move || apply_on_demand(&db, id, &result, mode)).await
}
