//! Import / export / clear-history / stats commands.
use std::fs::File;
use std::io::{BufWriter, Read, Write};

use tauri::State;

use crate::security;
use crate::{
    db::{validate_import_records, ExportCursor},
    require_feature, AppState, ClipboardRecord, FeatureId, StatsData,
};

/// Stream records as a JSON array directly to `path` (no full in-memory buffer).
#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = security::validate_json_io_path(&path, true)?;
    let file = File::create(&path).map_err(|e| format!("无法创建导出文件: {e}"))?;
    let mut w = BufWriter::new(file);
    w.write_all(b"[\n").map_err(|e| e.to_string())?;

    let page_size = 200;
    let mut cursor: Option<ExportCursor> = None;
    let mut first = true;
    loop {
        let batch = state
            .db
            .get_records_for_export_page(page_size, cursor.as_ref())
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
        cursor = batch.last().map(|record| ExportCursor {
            is_pinned: record.is_pinned,
            updated_at: record.updated_at.clone(),
            id: record.id,
        });
    }

    w.write_all(b"\n]\n").map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Import records from the renderer. The payload travels as a raw JSON string
/// so size/count validation runs BEFORE Tauri materializes a `Vec<ClipboardRecord>`
/// (a compromised webview could otherwise drive a multi-hundred-MB allocation
/// through argument deserialization alone).
#[tauri::command(rename_all = "snake_case")]
pub async fn import_data(state: State<'_, AppState>, records_json: String) -> Result<i32, String> {
    const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
    if records_json.len() > MAX_IMPORT_BYTES {
        return Err("导入内容过大（上限 64MB）".into());
    }
    let records: Vec<ClipboardRecord> =
        serde_json::from_str(&records_json).map_err(|e| format!("导入内容格式不正确: {e}"))?;
    validate_import_records(&records)?;
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
    // Cap import size to limit memory DoS from huge malicious files.
    const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
    let mut text = String::new();
    std::fs::File::open(&path)
        .map_err(|e| format!("无法读取备份文件: {e}"))?
        .take((MAX_IMPORT_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| format!("无法读取备份文件: {e}"))?;
    if text.len() > MAX_IMPORT_BYTES {
        return Err("备份文件过大（上限 64MB）".into());
    }
    let records: Vec<ClipboardRecord> =
        serde_json::from_str(&text).map_err(|e| format!("备份文件格式不正确: {e}"))?;
    validate_import_records(&records)?;
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
