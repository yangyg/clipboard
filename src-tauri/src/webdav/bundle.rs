//! Sync manifest + JSONL bundle (de)serialization and syncable filtering.
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::{validate_import_records, ClipboardDb, ExportCursor};
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

/// A deletion marker: content with this hash was explicitly deleted at
/// `deleted_at` (RFC3339). Recipients move their older copies to the trash;
/// a strictly newer local copy (deliberate re-copy) wins and supersedes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneEntry {
    pub hash: String,
    pub deleted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    pub version: u32,
    pub protocol: String,
    pub updated_at: String,
    pub device_id: String,
    #[serde(default)]
    pub entries: Vec<ManifestEntry>,
    /// Deletion tombstones (additive; old clients ignore unknown fields and
    /// simply keep their records until they upgrade).
    #[serde(default)]
    pub tombstones: Vec<TombstoneEntry>,
    /// device_id → newest tombstone `deleted_at` that device has applied.
    /// Used to garbage-collect tombstones only once every device has seen them.
    #[serde(default)]
    pub device_acks: std::collections::HashMap<String, String>,
    /// device_id → display name, so recipients can label record origins.
    /// Additive field: older clients ignore it and keep syncing normally.
    #[serde(default)]
    pub device_names: std::collections::HashMap<String, String>,
}

impl SyncManifest {
    pub fn empty(device_id: &str) -> Self {
        Self {
            version: 2,
            protocol: PROTOCOL.to_string(),
            updated_at: Utc::now().to_rfc3339(),
            device_id: device_id.to_string(),
            entries: vec![],
            tombstones: vec![],
            device_acks: std::collections::HashMap::new(),
            device_names: std::collections::HashMap::new(),
        }
    }
}

/// Merge local + remote tombstone candidates into one map (hash → latest
/// `deleted_at`). `local` is `(hash, deleted_at)` pairs already filtered by
/// the sync_sensitive policy.
pub fn merge_tombstone_candidates(
    local: &[(String, String)],
    remote: &[TombstoneEntry],
) -> std::collections::HashMap<String, String> {
    let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (hash, deleted_at) in local {
        out.entry(hash.clone())
            .and_modify(|t| {
                if deleted_at > t {
                    *t = deleted_at.clone();
                }
            })
            .or_insert_with(|| deleted_at.clone());
    }
    for t in remote {
        out.entry(t.hash.clone())
            .and_modify(|existing| {
                if t.deleted_at > *existing {
                    *existing = t.deleted_at.clone();
                }
            })
            .or_insert_with(|| t.deleted_at.clone());
    }
    out
}

/// Decide which tombstone candidates to publish against the final active
/// catalog (`hash → updated_at`), using the newer-wins rule:
/// - a catalog record strictly newer than the tombstone supersedes it
///   (`prune` — drop the stale local tombstone, keep the record);
/// - a catalog record at most as new as the tombstone is deleted
///   (`drop_active` — remove it from the catalog, publish the tombstone);
/// - no catalog record → publish the tombstone.
pub fn resolve_tombstones(
    active: &std::collections::HashMap<String, String>,
    candidates: &std::collections::HashMap<String, String>,
) -> (Vec<TombstoneEntry>, Vec<String>, Vec<String>) {
    let mut publish: Vec<TombstoneEntry> = Vec::new();
    let mut prune: Vec<String> = Vec::new();
    let mut drop_active: Vec<String> = Vec::new();
    for (hash, deleted_at) in candidates {
        match active.get(hash) {
            Some(updated_at) if updated_at > deleted_at => prune.push(hash.clone()),
            Some(_) => {
                drop_active.push(hash.clone());
                publish.push(TombstoneEntry {
                    hash: hash.clone(),
                    deleted_at: deleted_at.clone(),
                });
            }
            None => publish.push(TombstoneEntry {
                hash: hash.clone(),
                deleted_at: deleted_at.clone(),
            }),
        }
    }
    (publish, prune, drop_active)
}

/// Garbage-collect tombstones once every device in `acks` has applied them.
/// A device that never pushed an ack (or a fresh device) blocks GC, which is
/// the safe direction: missing acks must never cause a resurrection.
pub fn gc_tombstones(
    tombstones: Vec<TombstoneEntry>,
    acks: &std::collections::HashMap<String, String>,
) -> Vec<TombstoneEntry> {
    let Some(min_ack) = acks.values().min().cloned() else {
        return tombstones;
    };
    tombstones
        .into_iter()
        .filter(|t| t.deleted_at > min_ack)
        .collect()
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
    validate_import_records(&out)?;
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

pub fn filter_syncable(
    records: Vec<ClipboardRecord>,
    sync_sensitive: bool,
) -> Vec<ClipboardRecord> {
    records
        .into_iter()
        .filter(|r| sync_sensitive || !r.is_sensitive)
        .filter(|r| !r.is_trashed)
        .collect()
}

pub fn load_all_export(db: &ClipboardDb) -> Result<Vec<ClipboardRecord>, String> {
    let page = 200;
    let mut cursor: Option<ExportCursor> = None;
    let mut all = Vec::new();
    loop {
        let batch = db
            .get_records_for_export_page(page, cursor.as_ref())
            .map_err(|e| e.to_string())?;
        let len = batch.len();
        cursor = batch.last().map(|record| ExportCursor {
            is_pinned: record.is_pinned,
            updated_at: record.updated_at.clone(),
            id: record.id,
        });
        all.extend(batch);
        if len < page as usize {
            break;
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::{
        gc_tombstones, merge_tombstone_candidates, resolve_tombstones, SyncManifest, TombstoneEntry,
    };
    use std::collections::HashMap;

    fn tomb(hash: &str, deleted_at: &str) -> TombstoneEntry {
        TombstoneEntry {
            hash: hash.to_string(),
            deleted_at: deleted_at.to_string(),
        }
    }

    #[test]
    fn manifest_round_trips_tombstones_and_acks() {
        let mut acks = HashMap::new();
        acks.insert("dev-a".to_string(), "2026-02-01T00:00:00Z".to_string());
        let manifest = SyncManifest {
            version: 2,
            protocol: super::PROTOCOL.to_string(),
            updated_at: "2026-02-02T00:00:00Z".to_string(),
            device_id: "dev-a".to_string(),
            entries: vec![],
            tombstones: vec![tomb("h1", "2026-01-30T00:00:00Z")],
            device_acks: acks.clone(),
            device_names: HashMap::new(),
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: SyncManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tombstones, manifest.tombstones);
        assert_eq!(parsed.device_acks, acks);
        assert_eq!(parsed.version, 2);
    }

    #[test]
    fn legacy_manifest_without_new_fields_parses() {
        let json = r#"{
            "version": 1,
            "protocol": "clipvault-webdav-v1",
            "updated_at": "2026-01-01T00:00:00Z",
            "device_id": "legacy",
            "entries": []
        }"#;
        let parsed: SyncManifest = serde_json::from_str(json).unwrap();
        assert!(parsed.tombstones.is_empty());
        assert!(parsed.device_acks.is_empty());
    }

    #[test]
    fn merge_candidates_keeps_latest_deleted_at() {
        let local = vec![
            ("h1".to_string(), "2026-01-02T00:00:00Z".to_string()),
            ("h2".to_string(), "2026-01-01T00:00:00Z".to_string()),
        ];
        let remote = vec![
            tomb("h1", "2026-01-01T00:00:00Z"),
            tomb("h2", "2026-01-03T00:00:00Z"),
        ];
        let merged = merge_tombstone_candidates(&local, &remote);
        assert_eq!(merged["h1"], "2026-01-02T00:00:00Z");
        assert_eq!(merged["h2"], "2026-01-03T00:00:00Z");
    }

    #[test]
    fn resolve_tombstones_handles_supersede_drop_and_missing() {
        let active = HashMap::from([
            ("newer".to_string(), "2026-03-01T00:00:00Z".to_string()),
            ("stale".to_string(), "2026-01-01T00:00:00Z".to_string()),
        ]);
        let candidates = HashMap::from([
            ("newer".to_string(), "2026-02-01T00:00:00Z".to_string()),
            ("stale".to_string(), "2026-02-01T00:00:00Z".to_string()),
            ("gone".to_string(), "2026-02-01T00:00:00Z".to_string()),
        ]);

        let (publish, prune, drop_active) = resolve_tombstones(&active, &candidates);

        assert_eq!(prune, vec!["newer"]);
        assert_eq!(drop_active, vec!["stale"]);
        let mut published: Vec<&str> = publish.iter().map(|t| t.hash.as_str()).collect();
        published.sort_unstable();
        assert_eq!(published, vec!["gone", "stale"]);
    }

    #[test]
    fn gc_requires_every_device_ack() {
        let ts = vec![
            tomb("old", "2026-01-01T00:00:00Z"),
            tomb("newer", "2026-03-01T00:00:00Z"),
        ];
        // No acks → keep everything (safe direction).
        assert_eq!(gc_tombstones(ts.clone(), &HashMap::new()).len(), 2);

        // All devices acked past the old tombstone → only the newer one remains.
        let acks = HashMap::from([
            ("a".to_string(), "2026-02-01T00:00:00Z".to_string()),
            ("b".to_string(), "2026-02-01T00:00:00Z".to_string()),
        ]);
        let kept = gc_tombstones(ts.clone(), &acks);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].hash, "newer");

        // One device lagging → the old tombstone is retained.
        let lagging = HashMap::from([
            ("a".to_string(), "2026-02-01T00:00:00Z".to_string()),
            ("b".to_string(), "2025-12-01T00:00:00Z".to_string()),
        ]);
        assert_eq!(gc_tombstones(ts, &lagging).len(), 2);
    }

    #[test]
    fn bundle_round_trips_record_device_origin() {
        let now = chrono::Utc::now().to_rfc3339();
        let mut rec = crate::ClipboardRecord {
            id: 1,
            content: "origin payload".to_string(),
            content_type: "text".to_string(),
            source_app: "app.exe".to_string(),
            source_window: "win".to_string(),
            source_name: String::new(),
            source_device_id: "dev-remote".to_string(),
            hash: "h-origin".to_string(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: now.clone(),
            updated_at: now,
            tags: vec![],
            tag_colors: Vec::new(),
            content_html: None,
            media_path: None,
            thumb_path: None,
            width: None,
            height: None,
            media_abs: None,
            thumb_abs: None,
            content_len: None,
            alias: String::new(),
        };
        rec.media_abs = Some("C:\\secret".to_string());

        let bytes = super::serialize_bundle(&[rec]).unwrap();
        let parsed = super::parse_bundle(&bytes).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].source_device_id, "dev-remote");
        assert!(
            parsed[0].media_abs.is_none(),
            "absolute paths must not leak"
        );
    }

    #[test]
    fn bundle_legacy_line_without_origin_parses_to_empty() {
        let json = r#"{"id":0,"content":"legacy","content_type":"text","source_app":"","source_window":"","hash":"h-legacy","copy_count":0,"is_favorite":false,"is_pinned":false,"is_sensitive":false,"is_trashed":false,"auto_expire_at":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let parsed: crate::ClipboardRecord = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.source_device_id, "");
    }
}
