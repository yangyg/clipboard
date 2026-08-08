//! Pull / merge / push orchestration for Clipboard WebDAV sync.
//! Bundle (de)serialization lives in `bundle.rs`; media transfer in `media.rs`.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tracing::info;

use crate::db::{ClipboardDb, ImportSanitize};
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
    format!(
        "{}/{}",
        root.trim_end_matches('/'),
        rel.trim_start_matches('/')
    )
}

fn ensure_device_id(settings: &mut Settings) -> String {
    if settings.webdav_device_id.trim().is_empty() {
        settings.webdav_device_id = uuid::Uuid::new_v4().to_string();
    }
    settings.webdav_device_id.clone()
}

/// Persist settings with a fresh `webdav_last_sync_at` stamp, off the async worker.
async fn persist_last_sync(db: &Arc<ClipboardDb>, settings: &mut Settings) -> Result<(), String> {
    let db = Arc::clone(db);
    let mut next = settings.clone();
    next.webdav_last_sync_at = Some(Utc::now().to_rfc3339());
    let next = tokio::task::spawn_blocking(move || -> Result<Settings, String> {
        db.save_settings(&next).map_err(|e| e.to_string())?;
        Ok(next)
    })
    .await
    .map_err(|e| format!("WebDAV 保存设置任务失败: {e}"))??;
    *settings = next;
    Ok(())
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
) -> Result<RemoteState, String> {
    let manifest_rel = join_remote(root, MANIFEST_NAME);
    let manifest_remote = client.get_bytes_with_etag(&manifest_rel).await?;
    let (manifest, manifest_etag, manifest_exists) = match manifest_remote {
        Some(remote) => (
            serde_json::from_slice::<SyncManifest>(&remote.bytes)
                .map_err(|e| format!("解析 manifest.json 失败: {e}"))?,
            remote.etag,
            true,
        ),
        None => (SyncManifest::empty(device_id), None, false),
    };

    let bundle_rel = join_remote(root, BUNDLE_REL);
    let bundle_remote = client.get_bytes_with_etag(&bundle_rel).await?;
    let (records, bundle_etag, bundle_exists, bundle_bytes) = match bundle_remote {
        Some(remote) => {
            let bytes = remote.bytes;
            // Bundle parse is pure CPU over up-to-64MB — run off the async worker.
            let (records, bytes) = tokio::task::spawn_blocking(move || -> Result<_, String> {
                let records = parse_bundle(&bytes)?;
                Ok((records, bytes))
            })
            .await
            .map_err(|e| format!("解析 bundle 任务失败: {e}"))??;
            (records, remote.etag, true, Some(bytes))
        }
        None => (Vec::new(), None, false, None),
    };
    Ok(RemoteState {
        manifest,
        records,
        manifest_etag,
        manifest_exists,
        bundle_etag,
        bundle_exists,
        bundle_bytes,
    })
}

struct RemoteState {
    manifest: SyncManifest,
    records: Vec<crate::ClipboardRecord>,
    manifest_etag: Option<String>,
    manifest_exists: bool,
    bundle_etag: Option<String>,
    bundle_exists: bool,
    bundle_bytes: Option<Vec<u8>>,
}

pub async fn webdav_test_connection(settings: &Settings) -> Result<(), String> {
    let client = client_from_settings(settings)?;
    client.test_connection().await
}

pub async fn webdav_pull(
    db: &Arc<ClipboardDb>,
    settings: &mut Settings,
) -> Result<WebDavSyncResult, String> {
    let device_id = ensure_device_id(settings);
    let client = client_from_settings(settings)?;
    let root = remote_root(settings);
    let media_root = db.media_root().to_path_buf();
    media::ensure_dirs(&media_root).map_err(|e| e.to_string())?;

    let state = fetch_remote_state(&client, &root, &device_id).await?;
    let manifest = state.manifest;
    let mut records = state.records;
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
    let sanitize = ImportSanitize::from(&*settings);
    // The merge is a full-content transaction over the pulled bundle — run it
    // off the async worker so large imported sets don't hold a Tokio executor thread.
    let merge_db = Arc::clone(db);
    let (pulled, merged, tags_pulled) = tokio::task::spawn_blocking(move || {
        merge_db
            .import_records_with_merge(&records, max, Some(sanitize))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("WebDAV 导入任务失败: {e}"))??;
    persist_last_sync(db, settings).await?;

    info!(
        "WebDAV pull: new={pulled} merged={merged} tags={tags_pulled} media_dl={media_downloaded}"
    );
    Ok(WebDavSyncResult {
        pulled,
        pushed: 0,
        merged,
        tags_pulled,
        tags_pushed: 0,
        media_downloaded,
        media_uploaded: 0,
        media_skipped: 0,
    })
}

pub async fn webdav_push(
    db: &Arc<ClipboardDb>,
    settings: &mut Settings,
) -> Result<WebDavSyncResult, String> {
    let device_id = ensure_device_id(settings);
    let client = client_from_settings(settings)?;
    let root = remote_root(settings);
    let media_root = db.media_root().to_path_buf();

    client.ensure_collection(&root).await?;
    client
        .ensure_collection(&join_remote(&root, "records"))
        .await?;
    client
        .ensure_collection(&join_remote(&root, "media"))
        .await?;
    client
        .ensure_collection(&join_remote(&root, "media/thumbs"))
        .await?;

    let RemoteState {
        manifest: remote_manifest,
        records: remote_records,
        manifest_etag,
        manifest_exists,
        bundle_etag,
        bundle_exists,
        bundle_bytes,
    } = fetch_remote_state(&client, &root, &device_id).await?;
    let remote_entry_map: HashMap<String, ManifestEntry> = remote_manifest
        .entries
        .into_iter()
        .map(|e| (e.hash.clone(), e))
        .collect();
    let remote_tags: HashMap<String, Vec<String>> = remote_records
        .iter()
        .map(|r| (r.hash.clone(), r.tags.clone()))
        .collect();

    // Full-content export (content + content_html + tags for every record) is the
    // heaviest DB read in the app — keep it off the async worker.
    let load_db = Arc::clone(db);
    let local = filter_syncable(
        tokio::task::spawn_blocking(move || load_all_export(&load_db))
            .await
            .map_err(|e| format!("WebDAV 加载本地记录任务失败: {e}"))??,
        settings.webdav_sync_sensitive,
    );
    let local_by_hash: HashMap<String, crate::ClipboardRecord> =
        local.iter().map(|r| (r.hash.clone(), r.clone())).collect();

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

    // Tags diff vs the previous remote snapshot: count records whose published
    // tag set differs (new records count when they carry any tag). Independent
    // of `pushed` so a message can separate content changes from tag changes.
    let mut tags_pushed = 0;
    for (hash, rec) in &catalog {
        match remote_tags.get(hash) {
            None => {
                if !rec.tags.is_empty() {
                    tags_pushed += 1;
                }
            }
            Some(remote) => {
                let mut a = rec.tags.clone();
                let mut b = remote.clone();
                a.sort_unstable();
                b.sort_unstable();
                if a != b {
                    tags_pushed += 1;
                }
            }
        }
    }

    let mut entries: Vec<ManifestEntry> = catalog.values().map(record_to_entry).collect();
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let mut records: Vec<crate::ClipboardRecord> = catalog.into_values().collect();
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let bundle_payload = tokio::task::spawn_blocking(move || serialize_bundle(&records))
        .await
        .map_err(|e| format!("WebDAV 打包 bundle 任务失败: {e}"))??;
    let manifest = SyncManifest {
        version: 1,
        protocol: PROTOCOL.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        device_id,
        entries,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    let expected_manifest_etag = if manifest_exists {
        Some(
            manifest_etag
                .as_deref()
                .ok_or("WebDAV manifest 缺少 ETag，拒绝无条件覆盖远端数据")?,
        )
    } else {
        None
    };
    let expected_bundle_etag = if bundle_exists {
        Some(
            bundle_etag
                .as_deref()
                .ok_or("WebDAV bundle 缺少 ETag，拒绝无条件覆盖远端数据")?,
        )
    } else {
        None
    };
    let written_bundle_etag = client
        .put_bytes_if_match(
            &join_remote(&root, BUNDLE_REL),
            bundle_payload,
            "application/x-ndjson",
            expected_bundle_etag,
        )
        .await?;
    if let Err(manifest_error) = client
        .put_bytes_if_match(
            &join_remote(&root, MANIFEST_NAME),
            manifest_bytes,
            "application/json",
            expected_manifest_etag,
        )
        .await
    {
        let rollback = match (bundle_bytes, written_bundle_etag.as_deref()) {
            (Some(previous), Some(etag)) => client
                .put_bytes_if_match(
                    &join_remote(&root, BUNDLE_REL),
                    previous,
                    "application/x-ndjson",
                    Some(etag),
                )
                .await
                .map(|_| ()),
            (None, Some(etag)) => {
                client
                    .delete_bytes_if_match(&join_remote(&root, BUNDLE_REL), etag)
                    .await
            }
            (Some(_), None) | (None, None) => Err("新 bundle 缺少 ETag，无法安全回滚".into()),
        };
        return match rollback {
            Ok(()) => Err(format!(
                "写入 manifest 失败，已回滚 bundle: {manifest_error}"
            )),
            Err(rollback_error) => Err(format!(
                "写入 manifest 失败且 bundle 回滚失败: {manifest_error}; {rollback_error}"
            )),
        };
    }

    persist_last_sync(db, settings).await?;

    info!(
        "WebDAV push: changed≈{pushed} tags={tags_pushed} media_up={media_uploaded} media_skip={media_skipped}"
    );
    Ok(WebDavSyncResult {
        pulled: 0,
        pushed,
        merged: 0,
        tags_pulled: 0,
        tags_pushed,
        media_downloaded: 0,
        media_uploaded,
        media_skipped,
    })
}

pub async fn webdav_sync(
    db: &Arc<ClipboardDb>,
    settings: &mut Settings,
) -> Result<WebDavSyncResult, String> {
    let pull = webdav_pull(db, settings).await?;
    let push = webdav_push(db, settings).await?;
    Ok(WebDavSyncResult {
        pulled: pull.pulled,
        pushed: push.pushed,
        merged: pull.merged,
        tags_pulled: pull.tags_pulled,
        tags_pushed: push.tags_pushed,
        media_downloaded: pull.media_downloaded,
        media_uploaded: push.media_uploaded,
        media_skipped: push.media_skipped,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WebDavSyncResult {
    pub pulled: i32,
    pub pushed: i32,
    pub merged: i32,
    pub tags_pulled: i32,
    pub tags_pushed: i32,
    pub media_downloaded: i32,
    pub media_uploaded: i32,
    pub media_skipped: i32,
}
