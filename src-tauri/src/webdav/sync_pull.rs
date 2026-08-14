//! WebDAV pull: fetch remote snapshot, tombstone-filter, download media, merge.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;

use crate::db::{ClipboardDb, ImportSanitize};
use crate::media;
use crate::Settings;

use super::bundle::record_to_entry;
use super::media::{download_media_if_needed, MEDIA_TRANSFER_CONCURRENCY};
use super::sync_common::{
    client_from_settings, ensure_device_id, fetch_remote_state, persist_last_sync, remote_root,
    WebDavSyncResult,
};
use super::sync_merge::filter_tombstoned;
use super::tags::pull_tags_snapshot;

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
    // Local explicit deletions must not resurrect from the remote snapshot: a
    // record this device has tombstoned was deliberately deleted, so it is
    // dropped unless the incoming copy is strictly newer than the deletion
    // (a deliberate re-copy on another device wins).
    let local_tombstones: HashMap<String, String> = {
        let load_db = Arc::clone(db);
        let rows = super::spawn_block("WebDAV 加载本地 tombstone", move || {
            load_db.get_sync_tombstones().map_err(|e| e.to_string())
        })
        .await?;
        rows.into_iter()
            .map(|(hash, deleted_at, _)| (hash, deleted_at))
            .collect()
    };
    if !local_tombstones.is_empty() {
        records = filter_tombstoned(records, &local_tombstones);
    }

    let entry_by_hash: HashMap<String, super::bundle::ManifestEntry> = manifest
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
    let (pulled, merged, tags_pulled, trashed) = super::spawn_block("WebDAV 导入", move || {
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
    .await?;
    if trashed > 0 {
        info!("WebDAV pull: applied {trashed} deletion tombstone(s)");
    }
    // Standalone tag sync: tag definitions merge LWW by `tags.updated_at`, so
    // they no longer ride the record bundle (and tag edits never rewrite every
    // linked record's `updated_at`).
    let tags_pulled_total = tags_pulled
        + pull_tags_snapshot(db, &client, &root)
            .await
            .map_err(|e| format!("WebDAV 标签同步失败: {e}"))?;
    persist_last_sync(db, settings).await?;

    info!(
        "WebDAV pull: new={pulled} merged={merged} tags={tags_pulled_total} media_dl={media_downloaded}"
    );
    Ok(WebDavSyncResult {
        pulled,
        pushed: 0,
        merged,
        tags_pulled: tags_pulled_total,
        tags_pushed: 0,
        media_downloaded,
        media_uploaded: 0,
        media_skipped: 0,
    })
}
