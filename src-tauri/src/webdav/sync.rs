//! Pull / merge / push orchestration for Clipboard WebDAV sync.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::db::ClipboardDb;
use crate::media;
use crate::ClipboardRecord;
use crate::Settings;

use super::client::WebDavClient;

const PROTOCOL: &str = "clipvault-webdav-v1";
const MANIFEST_NAME: &str = "manifest.json";
const BUNDLE_REL: &str = "records/bundle.jsonl";

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
    fn empty(device_id: &str) -> Self {
        Self {
            version: 1,
            protocol: PROTOCOL.to_string(),
            updated_at: Utc::now().to_rfc3339(),
            device_id: device_id.to_string(),
            entries: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WebDavSyncResult {
    pub pulled: i32,
    pub pushed: i32,
    pub merged: i32,
    pub media_downloaded: i32,
    pub media_uploaded: i32,
    pub media_skipped: i32,
    pub message: String,
}

fn remote_root(settings: &Settings) -> String {
    let p = settings.webdav_remote_path.trim().trim_matches('/');
    if p.is_empty() {
        "ClipVaultSync".into()
    } else {
        p.to_string()
    }
}

fn join_remote(root: &str, rel: &str) -> String {
    format!("{}/{}", root.trim_end_matches('/'), rel.trim_start_matches('/'))
}

fn ensure_device_id(settings: &mut Settings) -> String {
    if settings.webdav_device_id.trim().is_empty() {
        settings.webdav_device_id = uuid::Uuid::new_v4().to_string();
    }
    settings.webdav_device_id.clone()
}

fn client_from_settings(settings: &Settings) -> Result<WebDavClient, String> {
    if settings.webdav_url.trim().is_empty() {
        return Err("请先填写 WebDAV 服务器地址".into());
    }
    if settings.webdav_username.trim().is_empty() {
        return Err("请先填写 WebDAV 用户名".into());
    }
    WebDavClient::new(
        &settings.webdav_url,
        &settings.webdav_username,
        &settings.webdav_password,
    )
}

fn record_to_entry(rec: &ClipboardRecord) -> ManifestEntry {
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

fn strip_abs_paths(mut rec: ClipboardRecord) -> ClipboardRecord {
    rec.media_abs = None;
    rec.thumb_abs = None;
    rec.id = 0;
    rec
}

fn parse_bundle(bytes: &[u8]) -> Result<Vec<ClipboardRecord>, String> {
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

fn serialize_bundle(records: &[ClipboardRecord]) -> Result<Vec<u8>, String> {
    let mut buf = String::new();
    for rec in records {
        let clean = strip_abs_paths(rec.clone());
        let line = serde_json::to_string(&clean).map_err(|e| e.to_string())?;
        buf.push_str(&line);
        buf.push('\n');
    }
    Ok(buf.into_bytes())
}

fn filter_syncable(records: Vec<ClipboardRecord>, sync_sensitive: bool) -> Vec<ClipboardRecord> {
    records
        .into_iter()
        .filter(|r| sync_sensitive || !r.is_sensitive)
        .filter(|r| !r.is_trashed)
        .collect()
}

fn load_all_export(db: &ClipboardDb) -> Result<Vec<ClipboardRecord>, String> {
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

async fn download_media_if_needed(
    client: &WebDavClient,
    root: &str,
    media_root: &Path,
    entry: &ManifestEntry,
) -> Result<bool, String> {
    if !entry.has_media {
        return Ok(false);
    }
    let mut downloaded = false;
    if let Some(rel) = entry.media_path.as_deref().filter(|p| !p.is_empty()) {
        let abs = media::absolute(media_root, rel);
        if !abs.exists() {
            let remote = join_remote(root, rel);
            if let Some(bytes) = client.get_bytes(&remote).await? {
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&abs, bytes).map_err(|e| e.to_string())?;
                downloaded = true;
            }
        }
    }
    if let Some(rel) = entry.thumb_path.as_deref().filter(|p| !p.is_empty()) {
        let abs = media::absolute(media_root, rel);
        if !abs.exists() {
            let remote = join_remote(root, rel);
            if let Some(bytes) = client.get_bytes(&remote).await? {
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&abs, bytes).map_err(|e| e.to_string())?;
                downloaded = true;
            }
        }
    }
    Ok(downloaded)
}

async fn upload_media_if_needed(
    client: &WebDavClient,
    root: &str,
    media_root: &Path,
    rec: &ClipboardRecord,
) -> Result<(bool, bool), String> {
    // returns (uploaded, skipped)
    let Some(media_rel) = rec.media_path.as_deref().filter(|p| !p.is_empty()) else {
        return Ok((false, false));
    };
    let abs = media::absolute(media_root, media_rel);
    if !abs.exists() {
        return Ok((false, false));
    }
    let remote = join_remote(root, media_rel);
    if client.exists(&remote).await? {
        // still ensure thumb if missing remotely
        let mut skipped = true;
        let mut uploaded = false;
        if let Some(thumb_rel) = rec.thumb_path.as_deref().filter(|p| !p.is_empty()) {
            let thumb_abs = media::absolute(media_root, thumb_rel);
            let thumb_remote = join_remote(root, thumb_rel);
            if thumb_abs.exists() && !client.exists(&thumb_remote).await? {
                let bytes = fs::read(&thumb_abs).map_err(|e| e.to_string())?;
                client
                    .put_bytes(&thumb_remote, bytes, "image/jpeg")
                    .await?;
                uploaded = true;
                skipped = false;
            }
        }
        return Ok((uploaded, skipped));
    }
    let bytes = fs::read(&abs).map_err(|e| e.to_string())?;
    let ct = if media_rel.ends_with(".png") {
        "image/png"
    } else if media_rel.ends_with(".jpg") || media_rel.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };
    client.put_bytes(&remote, bytes, ct).await?;
    if let Some(thumb_rel) = rec.thumb_path.as_deref().filter(|p| !p.is_empty()) {
        let thumb_abs = media::absolute(media_root, thumb_rel);
        if thumb_abs.exists() {
            let thumb_remote = join_remote(root, thumb_rel);
            if !client.exists(&thumb_remote).await? {
                let tbytes = fs::read(&thumb_abs).map_err(|e| e.to_string())?;
                client
                    .put_bytes(&thumb_remote, tbytes, "image/jpeg")
                    .await?;
            }
        }
    }
    Ok((true, false))
}

async fn fetch_remote_state(
    client: &WebDavClient,
    root: &str,
    device_id: &str,
) -> Result<(SyncManifest, Vec<ClipboardRecord>), String> {
    let manifest_rel = join_remote(root, MANIFEST_NAME);
    let bundle_rel = join_remote(root, BUNDLE_REL);

    let manifest = match client.get_bytes(&manifest_rel).await? {
        Some(bytes) => serde_json::from_slice::<SyncManifest>(&bytes)
            .map_err(|e| format!("解析 manifest.json 失败: {e}"))?,
        None => SyncManifest::empty(device_id),
    };
    let records = match client.get_bytes(&bundle_rel).await? {
        Some(bytes) => parse_bundle(&bytes)?,
        None => Vec::new(),
    };
    Ok((manifest, records))
}

pub async fn webdav_test_connection(settings: &Settings) -> Result<(), String> {
    let client = client_from_settings(settings)?;
    client.test_connection().await
}

pub async fn webdav_pull(db: &ClipboardDb, settings: &mut Settings) -> Result<WebDavSyncResult, String> {
    let device_id = ensure_device_id(settings);
    let client = client_from_settings(settings)?;
    let root = remote_root(settings);
    let media_root = db.media_root().to_path_buf();
    media::ensure_dirs(&media_root).map_err(|e| e.to_string())?;

    let (manifest, mut records) = fetch_remote_state(&client, &root, &device_id).await?;
    if !settings.webdav_sync_sensitive {
        records.retain(|r| !r.is_sensitive);
    }

    let entry_by_hash: HashMap<String, ManifestEntry> = manifest
        .entries
        .into_iter()
        .map(|e| (e.hash.clone(), e))
        .collect();

    let mut media_downloaded = 0;
    for rec in &records {
        let owned = entry_by_hash
            .get(&rec.hash)
            .cloned()
            .unwrap_or_else(|| record_to_entry(rec));
        if download_media_if_needed(&client, &root, &media_root, &owned).await? {
            media_downloaded += 1;
        }
    }

    let max = settings.max_records;
    let (pulled, merged) = db
        .import_records_with_merge(&records, max)
        .map_err(|e| e.to_string())?;

    settings.webdav_last_sync_at = Some(Utc::now().to_rfc3339());
    db.save_settings(settings).map_err(|e| e.to_string())?;

    info!(
        "WebDAV pull: new={pulled} merged={merged} media_dl={media_downloaded}"
    );
    Ok(WebDavSyncResult {
        pulled,
        pushed: 0,
        merged,
        media_downloaded,
        media_uploaded: 0,
        media_skipped: 0,
        message: format!("拉取完成：新增 {pulled}，合并 {merged}，下载媒体 {media_downloaded}"),
    })
}

pub async fn webdav_push(db: &ClipboardDb, settings: &mut Settings) -> Result<WebDavSyncResult, String> {
    let device_id = ensure_device_id(settings);
    let client = client_from_settings(settings)?;
    let root = remote_root(settings);
    let media_root = db.media_root().to_path_buf();

    client.ensure_collection(&root).await?;
    client.ensure_collection(&join_remote(&root, "records")).await?;
    client.ensure_collection(&join_remote(&root, "media")).await?;
    client
        .ensure_collection(&join_remote(&root, "media/thumbs"))
        .await?;

    let (remote_manifest, remote_records) = fetch_remote_state(&client, &root, &device_id).await?;
    let remote_entry_map: HashMap<String, ManifestEntry> = remote_manifest
        .entries
        .into_iter()
        .map(|e| (e.hash.clone(), e))
        .collect();

    let local = filter_syncable(load_all_export(db)?, settings.webdav_sync_sensitive);
    let local_by_hash: HashMap<String, ClipboardRecord> = local
        .iter()
        .map(|r| (r.hash.clone(), r.clone()))
        .collect();

    // Add-only: keep remote-only records in the published catalog
    let mut catalog: HashMap<String, ClipboardRecord> = HashMap::new();
    for r in remote_records {
        if settings.webdav_sync_sensitive || !r.is_sensitive {
            catalog.insert(r.hash.clone(), strip_abs_paths(r));
        }
    }
    for r in local {
        catalog.insert(r.hash.clone(), strip_abs_paths(r));
    }

    let mut media_uploaded = 0;
    let mut media_skipped = 0;
    let mut pushed = 0;

    for rec in catalog.values() {
        // Only upload media for records we have locally on disk
        if local_by_hash.contains_key(&rec.hash) {
            let (up, skip) = upload_media_if_needed(&client, &root, &media_root, rec).await?;
            if up {
                media_uploaded += 1;
            }
            if skip {
                media_skipped += 1;
            }
        }

        let remote_entry = remote_entry_map.get(&rec.hash);
        let needs_push = match remote_entry {
            None => true,
            Some(e) => e.updated_at.as_str() < rec.updated_at.as_str(),
        };
        if needs_push {
            pushed += 1;
        }
    }

    let mut entries: Vec<ManifestEntry> = catalog.values().map(record_to_entry).collect();
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let mut records: Vec<ClipboardRecord> = catalog.into_values().collect();
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let bundle_bytes = serialize_bundle(&records)?;
    client
        .put_bytes(
            &join_remote(&root, BUNDLE_REL),
            bundle_bytes,
            "application/x-ndjson",
        )
        .await?;

    let manifest = SyncManifest {
        version: 1,
        protocol: PROTOCOL.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        device_id,
        entries,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    client
        .put_bytes(
            &join_remote(&root, MANIFEST_NAME),
            manifest_bytes,
            "application/json",
        )
        .await?;

    settings.webdav_last_sync_at = Some(Utc::now().to_rfc3339());
    db.save_settings(settings).map_err(|e| e.to_string())?;

    info!(
        "WebDAV push: changed≈{pushed} media_up={media_uploaded} media_skip={media_skipped}"
    );
    Ok(WebDavSyncResult {
        pulled: 0,
        pushed,
        merged: 0,
        media_downloaded: 0,
        media_uploaded,
        media_skipped,
        message: format!(
            "推送完成：变更约 {pushed}，上传媒体 {media_uploaded}，跳过已有媒体 {media_skipped}"
        ),
    })
}

pub async fn webdav_sync(db: &ClipboardDb, settings: &mut Settings) -> Result<WebDavSyncResult, String> {
    let pull = webdav_pull(db, settings).await?;
    let push = webdav_push(db, settings).await?;
    Ok(WebDavSyncResult {
        pulled: pull.pulled,
        pushed: push.pushed,
        merged: pull.merged,
        media_downloaded: pull.media_downloaded,
        media_uploaded: push.media_uploaded,
        media_skipped: push.media_skipped,
        message: format!(
            "同步完成：新增 {}，合并 {}，推送变更约 {}，媒体 ↓{} ↑{}（跳过 {}）",
            pull.pulled,
            pull.merged,
            push.pushed,
            pull.media_downloaded,
            push.media_uploaded,
            push.media_skipped
        ),
    })
}
