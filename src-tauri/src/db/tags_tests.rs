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
