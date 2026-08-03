//! Pull / merge / push orchestration for Clipboard WebDAV sync.
//! Bundle (de)serialization lives in `bundle.rs`; media transfer in `media.rs`.

use std::collections::HashMap;

use chrono::Utc;
use tracing::info;

use crate::db::ClipboardDb;
use crate::media;
use crate::Settings;

use super::bundle::{
    filter_syncable, load_all_export, parse_bundle, record_to_entry, serialize_bundle,
    strip_abs_paths, ManifestEntry, SyncManifest, BUNDLE_REL, MANIFEST_NAME, PROTOCOL,
};
use super::client::WebDavClient;
use super::media::{download_media_if_needed, upload_media_if_needed};

fn remote_root(settings: &Settings) -> String {
    let p = settings.webdav_remote_path.trim().trim_matches('/');
    if p.is_empty() {
        "ClipVaultSync".into()
    } else {
        p.to_string()
    }
}

/// Join a remote path segment onto the sync root, tolerating stray slashes.
pub(super) fn join_remote(root: &str, rel: &str) -> String {
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

async fn fetch_remote_state(
    client: &WebDavClient,
    root: &str,
    device_id: &str,
) -> Result<(SyncManifest, Vec<crate::ClipboardRecord>), String> {
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
    let local_by_hash: HashMap<String, crate::ClipboardRecord> = local
        .iter()
        .map(|r| (r.hash.clone(), r.clone()))
        .collect();

    // Add-only: keep remote-only records in the published catalog. On a hash
    // collision (same content) keep the NEWER updated_at — a push without a
    // prior pull must not roll back a fresher timestamp published by another
    // device (a local copy would silently regress it).
    let mut catalog: HashMap<String, crate::ClipboardRecord> = HashMap::new();
    for r in remote_records {
        if settings.webdav_sync_sensitive || !r.is_sensitive {
            catalog.insert(r.hash.clone(), strip_abs_paths(r));
        }
    }
    for r in local {
        let r = strip_abs_paths(r);
        let regress = catalog
            .get(&r.hash)
            .map(|existing| existing.updated_at.as_str() >= r.updated_at.as_str())
            .unwrap_or(false);
        if !regress {
            catalog.insert(r.hash.clone(), r);
        }
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

    let mut records: Vec<crate::ClipboardRecord> = catalog.into_values().collect();
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

#[derive(Debug, Clone, serde::Serialize)]
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
