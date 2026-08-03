//! Sync manifest + JSONL bundle (de)serialization and syncable filtering.
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::ClipboardDb;
use crate::ClipboardRecord;

pub const PROTOCOL: &str = "clipvault-webdav-v1";
pub const MANIFEST_NAME: &str = "manifest.json";
pub const BUNDLE_REL: &str = "records/bundle.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub hash: String,
    pub updated_at: String,
    #[serde(default)]
    pub has_media: bool,
    #[serde(default)]
    pub media_path: Option<String>,
    #[serde(default)]
    pub thumb_path: Option<String>,
    #[serde(default)]
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    pub version: u32,
    pub protocol: String,
    pub updated_at: String,
    pub device_id: String,
    #[serde(default)]
    pub entries: Vec<ManifestEntry>,
}

impl SyncManifest {
    pub fn empty(device_id: &str) -> Self {
        Self {
            version: 1,
            protocol: PROTOCOL.to_string(),
            updated_at: Utc::now().to_rfc3339(),
            device_id: device_id.to_string(),
            entries: vec![],
        }
    }
}

pub fn record_to_entry(rec: &ClipboardRecord) -> ManifestEntry {
    let has_media = rec
        .media_path
        .as_ref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    ManifestEntry {
        hash: rec.hash.clone(),
        updated_at: rec.updated_at.clone(),
        has_media,
        media_path: rec.media_path.clone(),
        thumb_path: rec.thumb_path.clone(),
        content_type: rec.content_type.clone(),
    }
}

pub fn strip_abs_paths(mut rec: ClipboardRecord) -> ClipboardRecord {
    rec.media_abs = None;
    rec.thumb_abs = None;
    rec.id = 0;
    rec
}

pub fn parse_bundle(bytes: &[u8]) -> Result<Vec<ClipboardRecord>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rec: ClipboardRecord = serde_json::from_str(line)
            .map_err(|e| format!("bundle.jsonl 第 {} 行解析失败: {e}", i + 1))?;
        out.push(strip_abs_paths(rec));
    }
    Ok(out)
}

pub fn serialize_bundle(records: &[ClipboardRecord]) -> Result<Vec<u8>, String> {
    let mut buf = String::new();
    for rec in records {
        let clean = strip_abs_paths(rec.clone());
        let line = serde_json::to_string(&clean).map_err(|e| e.to_string())?;
        buf.push_str(&line);
        buf.push('\n');
    }
    Ok(buf.into_bytes())
}

pub fn filter_syncable(records: Vec<ClipboardRecord>, sync_sensitive: bool) -> Vec<ClipboardRecord> {
    records
        .into_iter()
        .filter(|r| sync_sensitive || !r.is_sensitive)
        .filter(|r| !r.is_trashed)
        .collect()
}

pub fn load_all_export(db: &ClipboardDb) -> Result<Vec<ClipboardRecord>, String> {
    let page = 200;
    let mut offset = 0;
    let mut all = Vec::new();
    loop {
        let batch = db
            .get_records_for_export(page, offset)
            .map_err(|e| e.to_string())?;
        let len = batch.len();
        all.extend(batch);
        if len < page as usize {
            break;
        }
        offset += page;
    }
    Ok(all)
}
