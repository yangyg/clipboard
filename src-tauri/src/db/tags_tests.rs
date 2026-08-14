//! Tests for tag palette snapping + tag mutation side effects. Lives in its
//! own file (mirroring `schema_tests.rs`) to keep `tags.rs` under the
//! 800-line cap.
use super::tags::{nearest_palette_color, normalize_color_key, TAG_PALETTE};
use crate::db::{PageCursor, TagSyncRow, TAG_EPOCH_SENTINEL};
use crate::ClipboardDb;
use crate::ClipboardRecord;

#[test]
fn palette_has_12_unique_swatches() {
    assert_eq!(TAG_PALETTE.len(), 12);
    let mut keys: Vec<_> = TAG_PALETTE.iter().map(|c| normalize_color_key(c)).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 12);
}

#[test]
fn nearest_returns_exact_match() {
    assert_eq!(nearest_palette_color("#3B82F6"), "#3b82f6");
    assert_eq!(
        normalize_color_key(nearest_palette_color("  #22C55E  ")),
        "#22c55e"
    );
}

#[test]
fn nearest_snaps_off_palette_onto_wheel() {
    let palette: Vec<_> = TAG_PALETTE.iter().map(|c| normalize_color_key(c)).collect();
    for legacy in ["#0078d4", "#60cdff", "#34d399", "#fbbf24", "#a78bfa"] {
        let snapped = normalize_color_key(nearest_palette_color(legacy));
        assert!(
            palette.contains(&snapped),
            "{legacy} snapped to {snapped} which is off-palette"
        );
    }
}

#[test]
fn nearest_invalid_falls_back() {
    assert_eq!(nearest_palette_color("not-a-color"), TAG_PALETTE[0]);
    assert_eq!(nearest_palette_color(""), TAG_PALETTE[0]);
}

#[test]
fn tag_mutations_bump_record_updated_at() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_bump_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let rec = ClipboardRecord {
        id: 0,
        content: "abc".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-bump".into(),
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
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;
    let read_updated = || {
        db.get_records_for_export(10, 0)
            .unwrap()
            .remove(0)
            .updated_at
    };
    let before = read_updated();

    std::thread::sleep(std::time::Duration::from_millis(5));
    let tag_id = db.create_tag("bump-tag-1", "#ef4444").unwrap();
    db.add_tag_to_record(record_id, tag_id).unwrap();
    assert_ne!(read_updated(), before);

    std::thread::sleep(std::time::Duration::from_millis(5));
    db.remove_tag_from_record(record_id, tag_id).unwrap();
    assert_ne!(read_updated(), before);

    // Re-applying the same set must NOT bump (no spurious list reorder).
    std::thread::sleep(std::time::Duration::from_millis(5));
    let tag2 = db.create_tag("bump-tag-2", "#eab308").unwrap();
    db.set_record_tags(record_id, &[tag_id, tag2]).unwrap();
    let after_set = read_updated();
    db.set_record_tags(record_id, &[tag_id, tag2]).unwrap();
    assert_eq!(read_updated(), after_set);

    // Renaming a tag must NOT bump linked records — tag definitions sync
    // standalone (tags.json), so the record's own content is unchanged.
    std::thread::sleep(std::time::Duration::from_millis(5));
    let after_set = read_updated();
    db.update_tag(tag_id, "VIP", "#ef4444").unwrap();
    assert_eq!(read_updated(), after_set);

    // A color-only change must not bump either (and must not rebuild FTS).
    std::thread::sleep(std::time::Duration::from_millis(5));
    db.update_tag(tag_id, "VIP", "#22c55e").unwrap();
    assert_eq!(read_updated(), after_set);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn tag_row(name: &str, color: &str, is_auto: bool, updated_at: &str) -> TagSyncRow {
    TagSyncRow {
        name: name.to_string(),
        color: color.to_string(),
        is_auto,
        updated_at: updated_at.to_string(),
    }
}

#[test]
fn merge_tag_snapshot_adds_and_updates_lww() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_merge_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // Fresh tag definitions from another device.
    let stats = db
        .merge_tag_snapshot(
            &[
                tag_row("工作", "#22c55e", true, "2026-06-01T00:00:00Z"),
                tag_row("家庭", "#ef4444", false, "2026-06-02T00:00:00Z"),
            ],
            &[],
            "2026-06-02T00:00:00Z",
        )
        .unwrap();
    assert_eq!(stats.added, 2);
    assert_eq!(db.get_all_tags(None, false).unwrap().len(), 7); // 5 defaults + 2

    // Newer color wins; an older color is ignored.
    let stats = db
        .merge_tag_snapshot(
            &[tag_row("工作", "#6366f1", true, "2026-06-03T00:00:00Z")],
            &[],
            "2026-06-03T00:00:00Z",
        )
        .unwrap();
    assert_eq!(stats.changed, 1);
    let t = db
        .get_all_tags(None, false)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "工作")
        .unwrap();
    assert_eq!(t.color, "#6366f1");

    let stats = db
        .merge_tag_snapshot(
            &[tag_row("工作", "#ef4444", true, "2026-05-01T00:00:00Z")],
            &[],
            "2026-06-03T00:00:00Z",
        )
        .unwrap();
    assert_eq!(stats.changed, 0);
    let t = db
        .get_all_tags(None, false)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "工作")
        .unwrap();
    assert_eq!(t.color, "#6366f1");

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn merge_tag_snapshot_tombstone_deletes_tag() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_tomb_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // A tag that exists on this device (via import/record link).
    let now = chrono::Utc::now().to_rfc3339();
    let rec = ClipboardRecord {
        id: 0,
        content: "tomb target".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-tomb".into(),
        copy_count: 0,
        is_favorite: false,
        is_pinned: false,
        is_sensitive: false,
        is_trashed: false,
        auto_expire_at: None,
        created_at: now.clone(),
        updated_at: now,
        tags: vec!["临时".into()],
        tag_colors: vec![],
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
    db.import_records_with_merge(&[rec], 100, None).unwrap();

    // Remote deletes the tag with a tombstone newer than the local stamp.
    let stats = db
        .merge_tag_snapshot(
            &[],
            &[("临时".to_string(), "2099-01-01T00:00:00Z".to_string())],
            "2099-01-01T00:00:00Z",
        )
        .unwrap();
    assert_eq!(stats.deleted, 1);
    assert!(db
        .get_all_tags(None, false)
        .unwrap()
        .iter()
        .all(|t| t.name != "临时"));
    // The record is no longer searchable by the deleted tag name.
    assert!(db
        .search_records(
            "临时",
            10,
            0,
            None,
            false,
            None,
            None,
            true,
            PageCursor::default()
        )
        .unwrap()
        .is_empty());

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn merge_tag_snapshot_local_edit_wins_over_older_tombstone() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_local_edit_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // The tag is edited on this device AFTER another device deleted it — the
    // local edit is a deliberate re-create and must win.
    let stamp = "2026-07-01T00:00:00Z";
    db.merge_tag_snapshot(&[tag_row("重要", "#ef4444", false, stamp)], &[], stamp)
        .unwrap();
    let stats = db
        .merge_tag_snapshot(
            &[],
            &[("重要".to_string(), "2026-06-01T00:00:00Z".to_string())],
            "2026-06-01T00:00:00Z",
        )
        .unwrap();
    assert_eq!(stats.deleted, 0);
    assert!(db
        .get_all_tags(None, false)
        .unwrap()
        .iter()
        .any(|t| t.name == "重要"));

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tombstone_blocks_stale_bundle_resurrection() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_resurrect_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // The tag was deleted on another device AFTER this record snapshot was made.
    let stamp = "2026-05-01T00:00:00Z";
    db.merge_tag_snapshot(
        &[],
        &[("已删".to_string(), "2026-06-01T00:00:00Z".to_string())],
        "2026-06-01T00:00:00Z",
    )
    .unwrap();
    let rec = ClipboardRecord {
        id: 0,
        content: "stale bundle".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-stale".into(),
        copy_count: 0,
        is_favorite: false,
        is_pinned: false,
        is_sensitive: false,
        is_trashed: false,
        auto_expire_at: None,
        created_at: stamp.to_string(),
        updated_at: stamp.to_string(),
        tags: vec!["已删".into()],
        tag_colors: vec![],
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
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert!(
        exported[0].tags.is_empty(),
        "tombstoned tag must not re-link"
    );

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn merge_tag_snapshot_gc_removes_zero_link_leftovers() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_gc_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // A real (non-sentinel) zero-link tag that the remote snapshot doesn't know
    // and hasn't seen updated — the leftover of a rename/delete elsewhere.
    let stamp = "2026-01-10T00:00:00Z";
    db.merge_tag_snapshot(&[tag_row("旧名字", "#ef4444", false, stamp)], &[], stamp)
        .unwrap();
    let stats = db
        .merge_tag_snapshot(&[], &[], "2026-02-01T00:00:00Z")
        .unwrap();
    assert_eq!(stats.deleted, 1);
    assert!(db
        .get_all_tags(None, false)
        .unwrap()
        .iter()
        .all(|t| t.name != "旧名字"));

    // A zero-link tag with links elsewhere keeps the GC from touching it.
    let now = chrono::Utc::now().to_rfc3339();
    let rec = ClipboardRecord {
        id: 0,
        content: "gc guard".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-gc".into(),
        copy_count: 0,
        is_favorite: false,
        is_pinned: false,
        is_sensitive: false,
        is_trashed: false,
        auto_expire_at: None,
        created_at: now.clone(),
        updated_at: now,
        tags: vec!["在用".into()],
        tag_colors: vec![],
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
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let stats = db
        .merge_tag_snapshot(&[], &[], "2099-01-01T00:00:00Z")
        .unwrap();
    assert_eq!(stats.deleted, 0);
    assert!(db
        .get_all_tags(None, false)
        .unwrap()
        .iter()
        .any(|t| t.name == "在用"));

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn merge_tag_snapshot_never_gcs_sentinel_defaults() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_gc_sentinel_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    // Fresh DB: defaults are sentinel-stamped and must survive a remote pull
    // whose snapshot doesn't mention them (otherwise a first sync would delete
    // every built-in tag on a fresh install).
    let rows = db.get_tag_sync_rows().unwrap().0;
    assert!(rows.iter().all(|r| r.updated_at == TAG_EPOCH_SENTINEL));
    let stats = db
        .merge_tag_snapshot(&[], &[], "2099-01-01T00:00:00Z")
        .unwrap();
    assert_eq!(stats.deleted, 0);
    assert_eq!(db.get_all_tags(None, false).unwrap().len(), 5);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn add_auto_tags_by_name_merges_and_is_idempotent() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_ai_tag_merge_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let rec = ClipboardRecord {
        id: 0,
        content: "ai-tag content".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-ai-tag".into(),
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
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;

    // Pre-exists with a manual tag; AI must not remove it.
    let manual = db.create_tag("manual-keep", "#ef4444").unwrap();
    db.add_tag_to_record(record_id, manual).unwrap();

    let names = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    // First call adds new auto tags; blanks are skipped. Name dedup inside the
    // DB method is case-sensitive (consistent with import), so the two case
    // variants both land.
    let added = db
        .add_auto_tags_by_name(
            record_id,
            &names(&["  AI-go   ", "ai-go", "", "代码", "代码"]),
        )
        .unwrap();
    assert_eq!(added, 3, "distinct non-empty trimmed names count");

    let tags = db.get_record_tag_names(record_id).unwrap();
    assert!(tags.contains(&"manual-keep".to_string()), "manual kept");
    assert!(tags.contains(&"AI-go".to_string()) || tags.contains(&"AI-go".to_string()));
    assert!(tags.contains(&"ai-go".to_string()) || tags.contains(&"ai-go".to_string()));
    assert!(tags.contains(&"代码".to_string()));

    // New auto tags are created as is_auto.
    let conn = db.lock_write();
    let is_auto: bool = conn
        .query_row("SELECT is_auto FROM tags WHERE name = '代码'", [], |r| {
            r.get::<_, i32>(0)
        })
        .map(|v| v != 0)
        .unwrap();
    drop(conn);
    assert!(is_auto, "AI tags must be created as auto tags");

    // Re-adding the same names reports 0 new (no FTS/watermark churn).
    let again = db
        .add_auto_tags_by_name(record_id, &names(&["AI-go", "代码"]))
        .unwrap();
    assert_eq!(again, 0);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleting_tag_removes_it_from_full_text_search() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_fts_delete_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let record = ClipboardRecord {
        id: 0,
        content: "searchable content".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-fts-delete".into(),
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
    db.import_records_with_merge(&[record], 100, None).unwrap();
    let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;
    let tag_id = db.create_tag("stale-search-tag", "#ef4444").unwrap();
    db.add_tag_to_record(record_id, tag_id).unwrap();

    assert_eq!(
        db.search_records(
            "stale-search-tag",
            10,
            0,
            None,
            false,
            None,
            None,
            true,
            PageCursor::default(),
        )
        .unwrap()
        .len(),
        1
    );
    db.delete_tag(tag_id).unwrap();
    assert!(db
        .search_records(
            "stale-search-tag",
            10,
            0,
            None,
            false,
            None,
            None,
            true,
            PageCursor::default(),
        )
        .unwrap()
        .is_empty());

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn get_all_tags_serves_ttl_cache_and_invalidates_on_mutation() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_tag_cache_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let record = ClipboardRecord {
        id: 0,
        content: "cache test".into(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: "hash-tag-cache".into(),
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
    db.import_records_with_merge(&[record], 100, None).unwrap();
    let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;
    let tag_id = db.create_tag("cache-tag", "#ef4444").unwrap();
    db.add_tag_to_record(record_id, tag_id).unwrap();

    // Repeated identical reads return identical data (TTL cache hit).
    let first = db.get_all_tags(None, false).unwrap();
    let second = db.get_all_tags(None, false).unwrap();
    assert_eq!(first.len(), second.len());
    let names = |tags: &[crate::TagInfo]| tags.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&first), names(&second));

    // A mutation (create_tag) bumps the epoch; the next read sees the new tag.
    let before = db.get_all_tags(None, false).unwrap().len();
    db.create_tag("cache-bust", "#22c55e").unwrap();
    let after = db.get_all_tags(None, false).unwrap();
    assert_eq!(after.len(), before + 1);
    assert!(after.iter().any(|t| t.name == "cache-bust"));

    // Distinct filter keys are cached independently.
    let fav_a = db.get_all_tags(None, true).unwrap();
    let fav_b = db.get_all_tags(None, true).unwrap();
    assert_eq!(names(&fav_a), names(&fav_b));
    assert_eq!(fav_a.len(), db.get_all_tags(None, false).unwrap().len());

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}
