//! Import / export / clear-history / stats commands.
use std::fs::File;
use std::io::{BufWriter, Write};

use tauri::State;

use crate::security;
use crate::{require_feature, AppState, ClipboardRecord, FeatureId, StatsData};

/// Stream records as a JSON array directly to `path` (no full in-memory buffer).
#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = security::validate_json_io_path(&path, true)?;
    let file = File::create(&path).map_err(|e| format!("无法创建导出文件: {e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"[\n").map_err(|e| e.to_string())?;

    let page_size = 200;
    let mut offset = 0;
    let mut first = true;
    loop {
        let batch = state
            .db
            .get_records_for_export(page_size, offset)
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
pub async fn import_data(
    state: State<'_, AppState>,
    records: Vec<ClipboardRecord>,
) -> Result<i32, String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    state
        .db
        .import_records(&records, settings.max_records)
        .map_err(|e| e.to_string())
}

/// Read a JSON backup from disk (path from native dialog) and import with sanitization.
#[tauri::command]
pub async fn import_data_from_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<i32, String> {
    let path = security::validate_json_io_path(&path, false)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("无法读取备份文件: {e}"))?;
    // Cap import size to limit memory DoS from huge malicious files.
    const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
    if text.len() > MAX_IMPORT_BYTES {
        return Err("备份文件过大（上限 64MB）".into());
    }
    let records: Vec<ClipboardRecord> =
        serde_json::from_str(&text).map_err(|e| format!("备份文件格式不正确: {e}"))?;
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    state
        .db
        .import_records(&records, settings.max_records)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_non_favorite().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<StatsData, String> {
    require_feature(
        &(*state.db.get_settings().map_err(|e| e.to_string())?),
        FeatureId::Stats,
    )?;
    // Cleanup stays on the periodic background thread — stats is a hot UI poll.
    state.db.get_stats().map_err(|e| e.to_string())
}
