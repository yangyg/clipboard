//! WebDAV push: catalog LWW, tombstones, media upload, conditional PUT.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use tracing::info;

use crate::db::ClipboardDb;
use crate::Settings;

use super::bundle::{
    filter_syncable, gc_tombstones, load_all_export, record_to_entry, serialize_bundle,
    ManifestEntry, SyncManifest, BUNDLE_REL, MANIFEST_NAME, PROTOCOL,
};
use super::media::{upload_media_paths_if_needed, MEDIA_TRANSFER_CONCURRENCY};
use super::sync_common::{
    client_from_settings, ensure_device_id, fetch_remote_state, join_remote, persist_last_sync,
    remote_root, RemoteState, WebDavSyncResult,
};
use super::sync_merge::{apply_resolved_tombstones, merge_catalog};
use super::tags::push_tags_snapshot;

type UploadResult = Result<(bool, bool), String>;
type UploadTask = (String, tokio::task::JoinHandle<UploadResult>);

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
        super::spawn_block("WebDAV 加载本地记录", move || {
            load_all_export(&load_db)
        })
        .await?,
        settings.webdav_sync_sensitive,
    );
    // Only the hash set is needed for the upload fan-out below — keeping the
    // full cloned record map here multiplied peak memory by the whole content
    // set (H1: push held ~4-5x total content bytes in memory).
    let local_hashes: HashSet<String> = local.iter().map(|r| r.hash.clone()).collect();

    let mut catalog = merge_catalog(remote_records, local, settings.webdav_sync_sensitive);

    // --- Deletion tombstones (cross-device delete propagation) ---
    // Resolve local + remote tombstones against the final catalog with the
    // newer-wins rule, then GC once every device has acked them.
    let sync_sensitive = settings.webdav_sync_sensitive;
    let load_db = Arc::clone(db);
    let (local_tombstones, local_ack) = super::spawn_block("WebDAV 加载 tombstone", move || {
        let ts = load_db.get_sync_tombstones().map_err(|e| e.to_string())?;
        let ack = load_db.get_tombstone_ack().map_err(|e| e.to_string())?;
        Ok::<_, String>((ts, ack))
    })
    .await?;
    let local_tombstones: Vec<(String, String)> = local_tombstones
        .into_iter()
        // Sensitive tombstones follow the same policy as sensitive records.
        .filter(|(_, _, is_sensitive)| sync_sensitive || !is_sensitive)
        .map(|(hash, deleted_at, _)| (hash, deleted_at))
        .collect();

    let (mut tombstones, prune_local) =
        apply_resolved_tombstones(&mut catalog, &local_tombstones, &remote_manifest.tombstones);
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

    let bundle_payload =
        super::spawn_block("WebDAV 打包 bundle", move || serialize_bundle(&records)).await?;
    let mut device_names = settings.webdav_device_names.clone();
    let device_name = settings.webdav_device_name.trim();
    if !device_name.is_empty() {
        device_names.insert(device_id.clone(), device_name.to_string());
    }
    let manifest = SyncManifest {
        version: 2,
        protocol: PROTOCOL.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        device_id: device_id.clone(),
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

    // Standalone tag sync: publish the local tag definitions + tombstones as
    // tags.json (LWW-merged, conditional PUT). Tag edits no longer bump every
    // linked record's `updated_at`, so definitions flow through this file
    // instead of re-pushing the record bundle.
    let (tags_pushed_total, tags_pulled_on_push) =
        push_tags_snapshot(db, &client, &root, &device_id)
            .await
            .map_err(|e| format!("WebDAV 标签同步失败: {e}"))?;
    let tags_pushed = tags_pushed_total + tags_pushed;

    persist_last_sync(db, settings).await?;

    info!(
        "WebDAV push: changed≈{pushed} tags={tags_pushed} media_up={media_uploaded} media_skip={media_skipped}"
    );
    Ok(WebDavSyncResult {
        pulled: 0,
        pushed,
        merged: 0,
        tags_pulled: tags_pulled_on_push,
        tags_pushed,
        media_downloaded: 0,
        media_uploaded,
        media_skipped,
    })
}
