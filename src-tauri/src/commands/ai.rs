//! AI commands (feature-gated): connection test for the settings page.
use tauri::State;

use crate::ai::AiClient;
use crate::{require_feature, AppState, FeatureId};

#[tauri::command(rename_all = "snake_case")]
pub async fn ai_test_connection(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    require_feature(&settings, FeatureId::Ai)?;
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
