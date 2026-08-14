//! Pure merge helpers for WebDAV pull/push: tombstone filter, origin pick, catalog LWW.

use std::collections::HashMap;

use crate::ClipboardRecord;

use super::bundle::{
    merge_tombstone_candidates, resolve_tombstones, strip_abs_paths, TombstoneEntry,
};

/// Drop incoming records that a local deletion tombstone covers (incoming copy
/// not strictly newer than the deletion time). Keeps deliberate re-copies.
pub(super) fn filter_tombstoned(
    mut records: Vec<ClipboardRecord>,
    tombstones: &HashMap<String, String>,
) -> Vec<ClipboardRecord> {
    if tombstones.is_empty() {
        return records;
    }
    records.retain(|r| match tombstones.get(&r.hash) {
        Some(deleted_at) => r.updated_at.as_str() > deleted_at.as_str(),
        None => true,
    });
    records
}

/// Pick the non-empty device origin of the earlier-created candidate.
/// Deterministic: equal `created_at` keeps `existing`; an empty incoming value
/// never overrides a known origin.
pub(super) fn pick_origin(
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

/// Add-only catalog merge: remote-only rows stay; same-hash keeps the newer
/// `updated_at` and the earlier-created device origin.
pub(super) fn merge_catalog(
    remote_records: Vec<ClipboardRecord>,
    local: Vec<ClipboardRecord>,
    sync_sensitive: bool,
) -> HashMap<String, ClipboardRecord> {
    let mut catalog: HashMap<String, ClipboardRecord> = HashMap::new();
    for r in remote_records {
        if sync_sensitive || !r.is_sensitive {
            catalog.insert(r.hash.clone(), strip_abs_paths(r));
        }
    }
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
    catalog
}

/// Apply resolved tombstones to the catalog. Returns (publish list, hashes to
/// prune from the local tombstone table).
pub(super) fn apply_resolved_tombstones(
    catalog: &mut HashMap<String, ClipboardRecord>,
    local_tombstones: &[(String, String)],
    remote_tombstones: &[TombstoneEntry],
) -> (Vec<TombstoneEntry>, Vec<String>) {
    let active: HashMap<String, String> = catalog
        .iter()
        .map(|(hash, rec)| (hash.clone(), rec.updated_at.clone()))
        .collect();
    let candidates = merge_tombstone_candidates(local_tombstones, remote_tombstones);
    let (tombstones, prune_local, drop_active) = resolve_tombstones(&active, &candidates);
    for hash in &drop_active {
        catalog.remove(hash);
    }
    (tombstones, prune_local)
}

#[cfg(test)]
mod tests {
    use super::{filter_tombstoned, pick_origin};
    use crate::ClipboardRecord;
    use std::collections::HashMap;

    fn mk(hash: &str, updated_at: &str) -> ClipboardRecord {
        ClipboardRecord {
            id: 0,
            content: String::new(),
            content_type: "text".into(),
            source_app: String::new(),
            source_window: String::new(),
            source_name: String::new(),
            source_device_id: String::new(),
            hash: hash.to_string(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
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
        }
    }

    #[test]
    fn filter_tombstoned_drops_older_copies_keeps_newer_recopy() {
        let tombstones = HashMap::from([("h1".to_string(), "2026-02-01T00:00:00Z".to_string())]);
        let records = vec![
            mk("h1", "2026-01-01T00:00:00Z"), // stale copy → dropped
            mk("h1", "2026-03-01T00:00:00Z"), // deliberate re-copy → kept
            mk("h2", "2026-01-01T00:00:00Z"), // never tombstoned → kept
        ];
        let kept = filter_tombstoned(records, &tombstones);
        assert_eq!(kept.len(), 2);
        assert!(kept
            .iter()
            .any(|r| r.hash == "h1" && r.updated_at == "2026-03-01T00:00:00Z"));
        assert!(kept.iter().any(|r| r.hash == "h2"));
    }

    #[test]
    fn filter_tombstoned_keeps_everything_without_tombstones() {
        let records = vec![mk("h1", "2026-01-01T00:00:00Z")];
        let kept = filter_tombstoned(records, &HashMap::new());
        assert_eq!(kept.len(), 1);
    }

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
