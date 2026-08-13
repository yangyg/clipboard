//! Import / export / clear-history / stats commands.
use std::fs::File;
use std::io::{BufWriter, Read, Write};

use tauri::State;

use crate::security;
use crate::{
    db::{
        validate_import_records, ClipboardDb, ExportCursor, ImportSanitize, MAX_IMPORT_TOTAL_BYTES,
    },
    AppState, ClipboardRecord, FeatureId, StatsData,
};

use super::{require_feature_state, spawn_db};

/// Stream records as a JSON array directly to `path` (no full in-memory buffer).
#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = security::validate_json_io_path(&path, true)?;
    let db = state.db.clone();
    spawn_db(move || -> Result<(), String> {
        let file = File::create(&path).map_err(|e| format!("无法创建导出文件: {e}"))?;
        let mut w = BufWriter::new(file);
        w.write_all(b"[\n").map_err(|e| e.to_string())?;

        let page_size = 200;
        let mut cursor: Option<ExportCursor> = None;
        let mut first = true;
        loop {
            let batch = db
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
    })
    .await
}

/// Import records from the renderer. The payload travels as a raw JSON string;
/// Shared parse + validate + merge body for both import commands. `label`
/// prefixes user-facing error messages ("导入内容"/"备份文件"). The byte-size
/// gate runs BEFORE any deserialization (in the callers) so a compromised
/// webview cannot force a multi-hundred-MB allocation through argument
/// deserialization; parsing itself runs on the blocking pool.
async fn run_import(
    db: std::sync::Arc<ClipboardDb>,
    json: String,
    label: &'static str,
) -> Result<i32, String> {
    let settings = db.get_settings().map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || {
        let records: Vec<ClipboardRecord> =
            serde_json::from_str(&json).map_err(|e| format!("{label}格式不正确: {e}"))?;
        validate_import_records(&records)?;
        db.import_records(
            &records,
            settings.max_records,
            Some(ImportSanitize::from(&*settings)),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("导入任务失败: {e}"))?
}

/// Import a JSON payload in memory. The string arrives whole from IPC, so its
/// length is the only cap needed here (record-count validation still runs in
/// `validate_import_records`).
#[tauri::command(rename_all = "snake_case")]
pub async fn import_data(state: State<'_, AppState>, records_json: String) -> Result<i32, String> {
    if records_json.len() > MAX_IMPORT_TOTAL_BYTES {
        return Err("导入内容过大（上限 64MB）".into());
    }
    let db = state.db.clone();
    run_import(db, records_json, "导入内容").await
}

/// Read a JSON backup from disk (path from native dialog) and import with sanitization.
#[tauri::command]
pub async fn import_data_from_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<i32, String> {
    let path = security::validate_json_io_path(&path, false)?;
    let db = state.db.clone();
    // Cap import size to limit memory DoS from huge malicious files.
    let mut text = String::new();
    std::fs::File::open(&path)
        .map_err(|e| format!("无法读取备份文件: {e}"))?
        .take((MAX_IMPORT_TOTAL_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| format!("无法读取备份文件: {e}"))?;
    if text.len() > MAX_IMPORT_TOTAL_BYTES {
        return Err("备份文件过大（上限 64MB）".into());
    }
    run_import(db, text, "备份文件").await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.clone();
    spawn_db(move || db.clear_non_favorite()).await
}

/// Wipe all clipboard data (records incl. favorites/pinned/trash, media files,
/// tags, search/sync history, WebDAV tombstones + ack watermark). App settings
/// survive; no tombstones are written so the next WebDAV pull joins fresh.
#[tauri::command]
pub async fn clear_all_data(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.clone();
    spawn_db(move || db.clear_all_data()).await
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<StatsData, String> {
    require_feature_state(&state, FeatureId::Stats)?;
    // Cleanup stays on the periodic background thread — stats is a hot UI poll.
    let perf_start = std::time::Instant::now();
    let db = state.db.clone();
    let stats = spawn_db(move || db.get_stats()).await?;
    crate::perf::log_elapsed("get_stats", perf_start);
    Ok(stats)
}
