//! Tests for tag palette snapping + tag mutation side effects. Lives in its
//! own file (mirroring `schema_tests.rs`) to keep `tags.rs` under the
//! 500-line cap.
use super::tags::{nearest_palette_color, normalize_color_key, TAG_PALETTE};
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

    // Renaming a tag must bump every linked record.
    std::thread::sleep(std::time::Duration::from_millis(5));
    db.update_tag(tag_id, "VIP", "#ef4444").unwrap();
    assert_ne!(read_updated(), after_set);

    // A color-only change must bump too so colors reach other devices.
    std::thread::sleep(std::time::Duration::from_millis(5));
    let after_color = read_updated();
    db.update_tag(tag_id, "VIP", "#22c55e").unwrap();
    assert_ne!(read_updated(), after_color);

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
    let conn = db.conn.lock();
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
            None,
            None,
            None
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
            None,
            None,
            None
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
