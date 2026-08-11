//! Pull / merge / push orchestration for Clipboard WebDAV sync.
//! Bundle (de)serialization lives in `bundle.rs`; media transfer in `media.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use tracing::info;

use crate::db::{ClipboardDb, ImportSanitize};
use crate::media;
use crate::Settings;

use super::bundle::{
    filter_syncable, gc_tombstones, load_all_export, merge_tombstone_candidates, parse_bundle,
    record_to_entry, resolve_tombstones, serialize_bundle, strip_abs_paths, ManifestEntry,
    SyncManifest, BUNDLE_REL, MANIFEST_NAME, PROTOCOL,
};
use super::client::WebDavClient;
use super::media::{
    download_media_if_needed, upload_media_paths_if_needed, MEDIA_TRANSFER_CONCURRENCY,
};

type UploadResult = Result<(bool, bool), String>;
type UploadTask = (String, tokio::task::JoinHandle<UploadResult>);

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

/// Pick the non-empty device origin of the earlier-created candidate.
/// Deterministic: equal `created_at` keeps `existing`; an empty incoming value
/// never overrides a known origin.
fn pick_origin(
    existing_id: &str,
    existing_created: &str,
    incoming_id: &str,
    incoming_created: &str,
) -> String {
    match (existing_id.is_empty(), incoming_id.is_empty()) {
        (true, true) => String::new(),
        (false, true) => existing_id.to_string(),
        (true, false) => incoming_id.to_string(),
        (false, false) => {
            if incoming_created < existing_created {
                incoming_id.to_string()
            } else {
                existing_id.to_string()
            }
        }
    }
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
    // Learn device display names published by peers so record-origin badges can
    // resolve ids → names. Merged into settings and persisted with the sync
    // stamp below (pull is additive: local names are never removed).
    let remote_device_names = state.manifest.device_names.clone();
    let manifest = state.manifest;
    if !remote_device_names.is_empty() {
        let mut known = settings.webdav_device_names.clone();
        for (id, name) in &remote_device_names {
            if !name.trim().is_empty() {
                known.insert(id.clone(), name.clone());
            }
        }
        settings.webdav_device_names = known;
    }
    let remote_tombstones: Vec<(String, String)> = manifest
        .tombstones
        .iter()
        .map(|t| (t.hash.clone(), t.deleted_at.clone()))
        .collect();
    let mut records = state.records;
    if !settings.webdav_sync_sensitive {
        records.retain(|r| !r.is_sensitive);
    }

    let entry_by_hash: HashMap<String, ManifestEntry> = manifest
        .entries
        .into_iter()
        .map(|e| (e.hash.clone(), e))
        .collect();

    // Concurrent, bounded media downloads: sequential GETs made sync wall time
    // scale linearly with image count. Server-polite 6-way fan-out instead.
    let download_semaphore = Arc::new(tokio::sync::Semaphore::new(MEDIA_TRANSFER_CONCURRENCY));
    let mut download_tasks = Vec::new();
    for rec in &records {
        let owned = entry_by_hash
            .get(&rec.hash)
            .cloned()
            .unwrap_or_else(|| record_to_entry(rec));
        let client = client.clone();
        let root = root.to_string();
        let media_root = media_root.clone();
        let permit = download_semaphore.clone();
        download_tasks.push(tokio::spawn(async move {
            let _guard = permit.acquire_owned().await.map_err(|e| e.to_string())?;
            download_media_if_needed(&client, &root, &media_root, &owned).await
        }));
    }
    let mut media_downloaded = 0;
    for task in download_tasks {
        if task.await.map_err(|e| format!("媒体下载任务失败: {e}"))?? {
            media_downloaded += 1;
        }
    }

    let max = settings.max_records;
    let sanitize = ImportSanitize::from(&*settings);
    // The merge is a full-content transaction over the pulled bundle — run it
    // off the async worker so large imported sets don't hold a Tokio executor thread.
    let merge_db = Arc::clone(db);
    let (pulled, merged, tags_pulled, trashed) = tokio::task::spawn_blocking(move || {
        let (pulled, merged, tags_pulled) = merge_db
            .import_records_with_merge(&records, max, Some(sanitize))
            .map_err(|e| e.to_string())?;
        // Apply deletion tombstones after the merge so a record that was
        // deleted elsewhere is trashed (recoverably) on this device too.
        let (trashed, _ack) = merge_db
            .apply_remote_tombstones(&remote_tombstones)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>((pulled, merged, tags_pulled, trashed))
    })
    .await
    .map_err(|e| format!("WebDAV 导入任务失败: {e}"))??;
    if trashed > 0 {
        info!("WebDAV pull: applied {trashed} deletion tombstone(s)");
    }
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
    // Only the hash set is needed for the upload fan-out below — keeping the
    // full cloned record map here multiplied peak memory by the whole content
    // set (H1: push held ~4-5x total content bytes in memory).
    let local_hashes: HashSet<String> = local.iter().map(|r| r.hash.clone()).collect();

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
    // Same-hash candidates: newer `updated_at` wins content, but the device
    // origin follows the earlier `created_at` — a re-copy on another device
    // must never re-label where the record came from.
    for r in local {
        let r = strip_abs_paths(r);
        match catalog.get_mut(&r.hash) {
            None => {
                catalog.insert(r.hash.clone(), r);
            }
            Some(existing) => {
                let origin = pick_origin(
                    &existing.source_device_id,
                    &existing.created_at,
                    &r.source_device_id,
                    &r.created_at,
                );
                if existing.updated_at.as_str() >= r.updated_at.as_str() {
                    existing.source_device_id = origin;
                } else {
                    let mut next = r;
                    next.source_device_id = origin;
                    *existing = next;
                }
            }
        }
    }

    // --- Deletion tombstones (cross-device delete propagation) ---
    // Resolve local + remote tombstones against the final catalog with the
    // newer-wins rule, then GC once every device has acked them.
    let sync_sensitive = settings.webdav_sync_sensitive;
    let load_db = Arc::clone(db);
    let (local_tombstones, local_ack) = tokio::task::spawn_blocking(move || {
        let ts = load_db.get_sync_tombstones().map_err(|e| e.to_string())?;
        let ack = load_db.get_tombstone_ack().map_err(|e| e.to_string())?;
        Ok::<_, String>((ts, ack))
    })
    .await
    .map_err(|e| format!("WebDAV 加载 tombstone 任务失败: {e}"))??;
    let local_tombstones: Vec<(String, String)> = local_tombstones
        .into_iter()
        // Sensitive tombstones follow the same policy as sensitive records.
        .filter(|(_, _, is_sensitive)| sync_sensitive || !is_sensitive)
        .map(|(hash, deleted_at, _)| (hash, deleted_at))
        .collect();

    let active: HashMap<String, String> = catalog
        .iter()
        .map(|(hash, rec)| (hash.clone(), rec.updated_at.clone()))
        .collect();
    let candidates = merge_tombstone_candidates(&local_tombstones, &remote_manifest.tombstones);
    let (mut tombstones, prune_local, drop_active) = resolve_tombstones(&active, &candidates);
    for hash in &drop_active {
        catalog.remove(hash);
    }
    if !prune_local.is_empty() {
        // Superseded by a strictly newer active copy — drop the stale local
        // tombstone so a later deletion starts from a clean slate.
        let prune_db = Arc::clone(db);
        let hashes = prune_local;
        tokio::task::spawn_blocking(move || {
            for hash in &hashes {
                if let Err(e) = prune_db.remove_tombstone(hash) {
                    tracing::warn!("Failed to prune superseded tombstone {hash}: {e}");
                }
            }
        })
        .await
        .map_err(|e| format!("WebDAV tombstone 清理任务失败: {e}"))?;
    }
    let mut device_acks = remote_manifest.device_acks.clone();
    if let Some(ack) = &local_ack {
        device_acks.insert(device_id.clone(), ack.clone());
    }
    tombstones = gc_tombstones(tombstones, &device_acks);

    // Concurrent, bounded media uploads for records we hold locally. Tasks
    // receive only the media paths (not the whole record) so fan-out does not
    // clone content/HTML buffers.
    let upload_semaphore = Arc::new(tokio::sync::Semaphore::new(MEDIA_TRANSFER_CONCURRENCY));
    let mut upload_tasks: Vec<UploadTask> = Vec::new();
    for rec in catalog.values() {
        if !local_hashes.contains(&rec.hash) {
            continue;
        }
        let Some(media_rel) = rec.media_path.as_deref().filter(|p| !p.is_empty()) else {
            continue;
        };
        let client = client.clone();
        let root = root.to_string();
        let media_root = media_root.clone();
        let media_rel = media_rel.to_string();
        let thumb_rel = rec.thumb_path.clone();
        let permit = upload_semaphore.clone();
        upload_tasks.push((
            rec.hash.clone(),
            tokio::spawn(async move {
                let _guard = permit.acquire_owned().await.map_err(|e| e.to_string())?;
                upload_media_paths_if_needed(
                    &client,
                    &root,
                    &media_root,
                    &media_rel,
                    thumb_rel.as_deref(),
                )
                .await
            }),
        ));
    }
    let mut upload_results: HashMap<String, (bool, bool)> = HashMap::new();
    for (hash, task) in upload_tasks {
        let result = task.await.map_err(|e| format!("媒体上传任务失败: {e}"))??;
        upload_results.insert(hash, result);
    }

    let mut media_uploaded = 0;
    let mut media_skipped = 0;
    let mut pushed = 0;

    for rec in catalog.values() {
        if let Some((up, skip)) = upload_results.get(&rec.hash) {
            if *up {
                media_uploaded += 1;
            }
            if *skip {
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
    let mut device_names = settings.webdav_device_names.clone();
    let device_name = settings.webdav_device_name.trim();
    if !device_name.is_empty() {
        device_names.insert(device_id.clone(), device_name.to_string());
    }
    let manifest = SyncManifest {
        version: 2,
        protocol: PROTOCOL.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        device_id,
        entries,
        tombstones,
        device_acks,
        device_names,
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

#[cfg(test)]
mod tests {
    use super::pick_origin;

    #[test]
    fn origin_follows_earlier_creator() {
        assert_eq!(
            pick_origin(
                "dev-new",
                "2026-06-01T00:00:00Z",
                "dev-old",
                "2026-01-01T00:00:00Z"
            ),
            "dev-old"
        );
        assert_eq!(
            pick_origin(
                "dev-old",
                "2026-01-01T00:00:00Z",
                "dev-new",
                "2026-06-01T00:00:00Z"
            ),
            "dev-old"
        );
        // Equal created_at keeps the first-seen candidate deterministically.
        assert_eq!(
            pick_origin(
                "dev-a",
                "2026-01-01T00:00:00Z",
                "dev-b",
                "2026-01-01T00:00:00Z"
            ),
            "dev-a"
        );
    }

    #[test]
    fn empty_origin_never_erases_known_origin() {
        assert_eq!(
            pick_origin("dev-a", "2026-01-01T00:00:00Z", "", "2025-01-01T00:00:00Z"),
            "dev-a"
        );
        assert_eq!(
            pick_origin("", "2026-01-01T00:00:00Z", "dev-b", "2025-01-01T00:00:00Z"),
            "dev-b"
        );
        assert_eq!(
            pick_origin("", "2026-01-01T00:00:00Z", "", "2025-01-01T00:00:00Z"),
            ""
        );
    }
}
