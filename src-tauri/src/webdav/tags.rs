//! Standalone tag sync (tags.json).
//!
//! Tag definitions (name / color / is_auto) used to travel embedded in every
//! record's `tags` field, so a tag edit had to bump every linked record's
//! `updated_at` to reach other devices — which re-pushed the whole record
//! bundle for a change no record content depends on. This module moves tag
//! definitions into their own snapshot file, merged LWW by `tags.updated_at`,
//! so tag edits never touch `records.updated_at`.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::client::WebDavClient;
use super::sync::join_remote;
use crate::db::{ClipboardDb, TagMergeStats, TagSyncRow, TAG_EPOCH_SENTINEL};

pub const TAGS_REL: &str = "records/tags.json";
pub const TAGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSnapshot {
    pub name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub is_auto: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagTombstone {
    pub name: String,
    pub deleted_at: String,
}

/// Full snapshot of one device's tag definitions + deletion tombstones.
/// A full set (not a delta) so a merge never depends on seeing every
/// intermediate write, and a missing file on the remote simply means "no tags
/// published". Older clients that ignore this file are unaffected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsFile {
    pub version: u32,
    /// LWW stamp of the snapshot itself (max tag `updated_at`). Used by the
    /// conservative GC so local tags untouched since the snapshot can be
    /// dropped as leftovers of a rename / delete elsewhere.
    pub updated_at: String,
    /// Publishing device; ignored for merge (tags merge by name, LWW on the
    /// tag's own stamp, not the publisher).
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub tags: Vec<TagSnapshot>,
    #[serde(default)]
    pub tombstones: Vec<TagTombstone>,
}

impl TagsFile {
    /// Build a snapshot from the local tag definitions + tombstones.
    pub fn from_db(rows: &[TagSyncRow], tombstones: &[(String, String)]) -> Self {
        let mut updated_at = "1970-01-01T00:00:00Z".to_string();
        for r in rows {
            if r.updated_at.as_str() > updated_at.as_str() {
                updated_at = r.updated_at.clone();
            }
        }
        Self {
            version: TAGS_VERSION,
            updated_at,
            device_id: String::new(), // filled by the caller from settings
            tags: rows
                .iter()
                .map(|r| TagSnapshot {
                    name: r.name.clone(),
                    color: r.color.clone(),
                    is_auto: r.is_auto,
                    updated_at: r.updated_at.clone(),
                })
                .collect(),
            tombstones: tombstones
                .iter()
                .map(|(name, deleted_at)| TagTombstone {
                    name: name.clone(),
                    deleted_at: deleted_at.clone(),
                })
                .collect(),
        }
    }
}

/// Fetch the remote tags.json (None when absent / not yet published).
async fn fetch_remote_tags(
    client: &WebDavClient,
    root: &str,
) -> Result<(Option<TagsFile>, Option<String>), String> {
    let rel = join_remote(root, TAGS_REL);
    let Some(remote) = client.get_bytes_with_etag(&rel).await? else {
        return Ok((None, None));
    };
    let parsed = serde_json::from_slice::<TagsFile>(&remote.bytes)
        .map_err(|e| format!("解析 tags.json 失败: {e}"))?;
    Ok((Some(parsed), remote.etag))
}

/// Push path: fetch the remote tags.json first (if any), fold remote rows we
/// have not seen into the local union, then publish conditionally on the
/// remote etag so a concurrent writer is never silently overwritten.
/// Returns `(tags_pushed, remote_rows_learned)`.
pub async fn push_tags_snapshot(
    db: &Arc<ClipboardDb>,
    client: &WebDavClient,
    root: &str,
    device_id: &str,
) -> Result<(i32, i32), String> {
    let rel = join_remote(root, TAGS_REL);
    let (remote, remote_etag) = fetch_remote_tags(client, root).await?;

    // Load local rows + tombstones off the async worker (whole tags table).
    let db_local = Arc::clone(db);
    let (rows, tombstones) = tokio::task::spawn_blocking(move || {
        db_local.get_tag_sync_rows().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("WebDAV 加载标签任务失败: {e}"))??;

    let mut rows = rows;
    let mut tombstones = tombstones;
    let mut remote_learned = 0i32;
    if let Some(remote_file) = &remote {
        // Fold remote rows into the local union (LWW keeps the newer). The
        // published snapshot then can't lose definitions to a pull that races
        // this push, even if the conditional PUT has to retry.
        for remote_row in &remote_file.tags {
            match rows.iter_mut().find(|r| r.name == remote_row.name) {
                Some(local) if local.updated_at < remote_row.updated_at => {
                    local.color = remote_row.color.clone();
                    local.is_auto = remote_row.is_auto;
                    local.updated_at = remote_row.updated_at.clone();
                    remote_learned += 1;
                }
                Some(_) => {}
                None => {
                    rows.push(TagSyncRow {
                        name: remote_row.name.clone(),
                        color: remote_row.color.clone(),
                        is_auto: remote_row.is_auto,
                        updated_at: remote_row.updated_at.clone(),
                    });
                    remote_learned += 1;
                }
            }
        }
        // Re-publish deletion tombstones so a device pruning on our snapshot
        // does not resurrect a tag the remote deleted.
        for remote_tomb in &remote_file.tombstones {
            if !tombstones
                .iter()
                .any(|(n, d)| n == &remote_tomb.name && d >= &remote_tomb.deleted_at)
            {
                tombstones.push((remote_tomb.name.clone(), remote_tomb.deleted_at.clone()));
            }
        }
    }

    let mut file = TagsFile::from_db(&rows, &tombstones);
    // `device_id` is metadata, not merge input — strip it from the comparison
    // body so a file last written by another device doesn't count as a change
    // and re-PUT on every sync cycle.
    let mut file_for_cmp = file.clone();
    file_for_cmp.device_id = String::new();
    file.device_id = device_id.to_string();
    let body = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
    let body_for_cmp = serde_json::to_vec(&file_for_cmp).map_err(|e| e.to_string())?;

    // Publish only when the union actually differs from the remote file, so an
    // unchanged sync does not write on every tick. Both sides are
    // parsed-then-reserialized with device_id normalized away, so the
    // comparison is canonical regardless of remote whitespace or publisher.
    let needs_write = match &remote {
        Some(r) => {
            let mut r_cmp = r.clone();
            r_cmp.device_id = String::new();
            let remote_body = serde_json::to_vec(&r_cmp).map_err(|e| e.to_string())?;
            body_for_cmp != remote_body
        }
        None => true,
    };
    let pushed = if needs_write {
        client
            .put_bytes_if_match(&rel, body, "application/json", remote_etag.as_deref())
            .await?;
        // Count the definitions we actually changed on the wire. Local edits
        // carry a real (non-sentinel) stamp; legacy rows are excluded so a
        // first sync after upgrade doesn't report every tag as pushed.
        rows.iter()
            .filter(|r| r.updated_at != TAG_EPOCH_SENTINEL)
            .count() as i32
    } else {
        0
    };
    Ok((pushed, remote_learned))
}

/// Pull path: merge the remote tags.json into the local DB. `tags_pulled` is
/// `added + changed + deleted` from the merge. The remote file is not modified
/// on pull — a subsequent push publishes the union — so no etag is needed.
pub async fn pull_tags_snapshot(
    db: &Arc<ClipboardDb>,
    client: &WebDavClient,
    root: &str,
) -> Result<i32, String> {
    let (remote, _) = fetch_remote_tags(client, root).await?;
    let Some(remote) = remote else {
        return Ok(0);
    };
    let rows: Vec<TagSyncRow> = remote
        .tags
        .into_iter()
        .map(|t| TagSyncRow {
            name: t.name,
            color: t.color,
            is_auto: t.is_auto,
            updated_at: t.updated_at,
        })
        .collect();
    let tombstones: Vec<(String, String)> = remote
        .tombstones
        .into_iter()
        .map(|t| (t.name, t.deleted_at))
        .collect();
    let snapshot_updated_at = remote.updated_at.clone();
    let db_local = Arc::clone(db);
    let stats: TagMergeStats = tokio::task::spawn_blocking(move || {
        db_local
            .merge_tag_snapshot(&rows, &tombstones, &snapshot_updated_at)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("WebDAV 合并标签任务失败: {e}"))??;
    Ok(stats.added + stats.changed + stats.deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_file_round_trips_and_tolerates_legacy() {
        let mut file = TagsFile {
            version: TAGS_VERSION,
            updated_at: "2026-06-03T00:00:00Z".to_string(),
            device_id: "dev-1".to_string(),
            tags: vec![],
            tombstones: vec![],
        };
        file.tags.push(TagSnapshot {
            name: "部署".to_string(),
            color: "#22c55e".to_string(),
            is_auto: true,
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        });
        file.tombstones.push(TagTombstone {
            name: "旧标签".to_string(),
            deleted_at: "2026-06-02T00:00:00Z".to_string(),
        });
        let json = serde_json::to_string(&file).unwrap();
        let parsed: TagsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, TAGS_VERSION);
        assert_eq!(parsed.tags.len(), 1);
        assert_eq!(parsed.tags[0].name, "部署");
        assert_eq!(parsed.tombstones.len(), 1);

        // A remote file without the newer fields (older client) still parses.
        let legacy =
            r#"{"version":1,"updated_at":"2026-01-01T00:00:00Z","device_id":"old","tags":[]}"#;
        let parsed: TagsFile = serde_json::from_str(legacy).unwrap();
        assert!(parsed.tombstones.is_empty());
    }

    #[test]
    fn empty_snapshot_stamp_is_epoch() {
        let file = TagsFile::from_db(&[], &[]);
        assert_eq!(file.updated_at, "1970-01-01T00:00:00Z");
    }
}
