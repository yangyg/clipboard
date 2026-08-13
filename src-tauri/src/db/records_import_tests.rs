//! Tests for import/merge + export paging. Lives in its own file (mirroring
//! `schema_tests.rs`) to keep `records_import.rs` under the 500-line cap.
use super::records_import::MAX_IMPORT_CONTENT_BYTES;
use crate::db::{validate_import_records, ClipboardDb, ExportCursor, ImportSanitize};
use crate::ClipboardRecord;
use std::path::PathBuf;

fn temp_db() -> (ClipboardDb, PathBuf) {
    super::test_util::temp_db("records_import")
}

fn cleanup(dir: PathBuf) {
    super::test_util::cleanup(dir)
}

fn make_record(content: &str, hash: &str, tags: &[&str]) -> ClipboardRecord {
    make_record_at(content, hash, tags, chrono::Utc::now().to_rfc3339())
}

fn make_record_at(content: &str, hash: &str, tags: &[&str], stamp: String) -> ClipboardRecord {
    make_record_with_colors(content, hash, tags, &[], stamp)
}

fn make_record_with_colors(
    content: &str,
    hash: &str,
    tags: &[&str],
    colors: &[(&str, &str)],
    stamp: String,
) -> ClipboardRecord {
    ClipboardRecord {
        id: 0,
        content: content.to_string(),
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
        created_at: stamp.clone(),
        updated_at: stamp,
        tags: tags.iter().map(|s| s.to_string()).collect(),
        tag_colors: colors
            .iter()
            .map(|(n, c)| (n.to_string(), c.to_string()))
            .collect(),
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
fn import_creates_tags_and_links() {
    let (db, dir) = temp_db();
    db.import_records_with_merge(
        &[make_record("hello", "hash-1", &["重要", "链接"])],
        100,
        None,
    )
    .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(exported.len(), 1);
    let mut tags = exported[0].tags.clone();
    tags.sort();
    let mut want = vec!["链接".to_string(), "重要".to_string()];
    want.sort();
    assert_eq!(tags, want);
    cleanup(dir);
}

#[test]
fn import_merge_replaces_tags_when_incoming_has_tags() {
    let (db, dir) = temp_db();
    db.import_records_with_merge(&[make_record("same", "hash-x", &["重要"])], 100, None)
        .unwrap();
    db.import_records_with_merge(&[make_record("same", "hash-x", &["链接"])], 100, None)
        .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(exported.len(), 1);
    assert_eq!(exported[0].tags, ["链接"]);
    cleanup(dir);
}

#[test]
fn import_merge_preserves_local_tags_for_tagless_snapshot() {
    let (db, dir) = temp_db();
    db.import_records_with_merge(&[make_record("same", "hash-y", &["重要"])], 100, None)
        .unwrap();
    db.import_records_with_merge(&[make_record("same", "hash-y", &[])], 100, None)
        .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(exported[0].tags, ["重要"]);
    cleanup(dir);
}

#[test]
fn import_merge_preserves_newer_local_tags_against_older_snapshot() {
    let (db, dir) = temp_db();
    let t_older = "2026-01-02T00:00:00Z".to_string();
    let t_newer = "2026-02-02T00:00:00Z".to_string();
    db.import_records_with_merge(
        &[make_record_at("same", "hash-lww", &["重要"], t_newer)],
        100,
        None,
    )
    .unwrap();
    db.import_records_with_merge(
        &[make_record_at("same", "hash-lww", &["链接"], t_older)],
        100,
        None,
    )
    .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(exported[0].tags, ["重要"]);
    cleanup(dir);
}

#[test]
fn import_merge_applies_newer_snapshot_tags_over_stale_local() {
    let (db, dir) = temp_db();
    let t_older = "2026-01-02T00:00:00Z".to_string();
    let t_newer = "2026-02-02T00:00:00Z".to_string();
    db.import_records_with_merge(
        &[make_record_at("same", "hash-lww2", &["重要"], t_older)],
        100,
        None,
    )
    .unwrap();
    let (_, _, tc) = db
        .import_records_with_merge(
            &[make_record_at("same", "hash-lww2", &["链接"], t_newer)],
            100,
            None,
        )
        .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(exported[0].tags, ["链接"]);
    assert_eq!(tc, 1);
    cleanup(dir);
}

#[test]
fn import_applies_tag_colors_from_newer_snapshot() {
    let (db, dir) = temp_db();
    db.import_records_with_merge(
        &[make_record_with_colors(
            "color",
            "hash-c1",
            &["工作"],
            &[("工作", "#a855f7")],
            "2026-03-01T00:00:00Z".to_string(),
        )],
        100,
        None,
    )
    .unwrap();
    let tags = db.get_all_tags(None, false).unwrap();
    assert_eq!(
        tags.iter().find(|t| t.name == "工作").unwrap().color,
        "#a855f7"
    );
    cleanup(dir);
}

#[test]
fn import_color_only_change_updates_existing_tag_color() {
    let (db, dir) = temp_db();
    let t_older = "2026-01-02T00:00:00Z".to_string();
    let t_newer = "2026-02-02T00:00:00Z".to_string();
    db.import_records_with_merge(
        &[make_record_with_colors(
            "color",
            "hash-c2",
            &["工作"],
            &[("工作", "#ef4444")],
            t_older,
        )],
        100,
        None,
    )
    .unwrap();
    // Identical link set, newer snapshot, different color — color must still apply.
    let (_, _, tc) = db
        .import_records_with_merge(
            &[make_record_with_colors(
                "color",
                "hash-c2",
                &["工作"],
                &[("工作", "#22c55e")],
                t_newer,
            )],
            100,
            None,
        )
        .unwrap();
    assert_eq!(tc, 0);
    let tags = db.get_all_tags(None, false).unwrap();
    assert_eq!(
        tags.iter().find(|t| t.name == "工作").unwrap().color,
        "#22c55e"
    );
    cleanup(dir);
}

#[test]
fn import_older_snapshot_does_not_recolor_tag() {
    let (db, dir) = temp_db();
    let t_older = "2026-01-02T00:00:00Z".to_string();
    let t_newer = "2026-02-02T00:00:00Z".to_string();
    db.import_records_with_merge(
        &[make_record_with_colors(
            "color",
            "hash-c3",
            &["工作"],
            &[("工作", "#22c55e")],
            t_newer,
        )],
        100,
        None,
    )
    .unwrap();
    db.import_records_with_merge(
        &[make_record_with_colors(
            "color",
            "hash-c3",
            &["工作"],
            &[("工作", "#ef4444")],
            t_older,
        )],
        100,
        None,
    )
    .unwrap();
    let tags = db.get_all_tags(None, false).unwrap();
    assert_eq!(
        tags.iter().find(|t| t.name == "工作").unwrap().color,
        "#22c55e"
    );
    cleanup(dir);
}

#[test]
fn export_carries_tag_colors_in_bundle_payload() {
    let (db, dir) = temp_db();
    db.import_records_with_merge(
        &[make_record_with_colors(
            "color",
            "hash-c4",
            &["工作", "重要"],
            &[("工作", "#a855f7"), ("重要", "#ef4444")],
            "2026-03-01T00:00:00Z".to_string(),
        )],
        100,
        None,
    )
    .unwrap();
    let exported = db.get_records_for_export(10, 0).unwrap();
    let mut colors = exported[0].tag_colors.clone();
    colors.sort();
    assert_eq!(
        colors,
        vec![
            ("工作".to_string(), "#a855f7".to_string()),
            ("重要".to_string(), "#ef4444".to_string()),
        ]
    );
    cleanup(dir);
}

#[test]
fn tags_changed_counts_only_real_changes() {
    let (db, dir) = temp_db();
    // New record with tags → counts 1.
    let (_, _, tc) = db
        .import_records_with_merge(&[make_record("a", "hash-tc", &["重要"])], 100, None)
        .unwrap();
    assert_eq!(tc, 1);
    // Merge with identical tags → 0 (no spurious count).
    let (_, _, tc) = db
        .import_records_with_merge(&[make_record("a", "hash-tc", &["重要"])], 100, None)
        .unwrap();
    assert_eq!(tc, 0);
    // Merge with a changed tag set → 1.
    let (_, _, tc) = db
        .import_records_with_merge(&[make_record("a", "hash-tc", &["链接"])], 100, None)
        .unwrap();
    assert_eq!(tc, 1);
    // Merge with empty tags → 0 (preserves local, counts nothing).
    let (_, _, tc) = db
        .import_records_with_merge(&[make_record("a", "hash-tc", &[])], 100, None)
        .unwrap();
    assert_eq!(tc, 0);
    cleanup(dir);
}

#[test]
fn import_deduplicates_repeated_hashes_in_one_batch() {
    let (db, dir) = temp_db();
    let records = [
        make_record("same", "batch-duplicate", &[]),
        make_record("same", "batch-duplicate", &[]),
    ];

    let (imported, merged, _) = db.import_records_with_merge(&records, 100, None).unwrap();

    assert_eq!((imported, merged), (1, 1));
    assert_eq!(db.get_records_for_export(10, 0).unwrap().len(), 1);
    cleanup(dir);
}

#[test]
fn import_rejects_oversized_content() {
    let record = make_record(
        &"x".repeat(MAX_IMPORT_CONTENT_BYTES + 1),
        "oversized-content",
        &[],
    );

    let error = validate_import_records(&[record]).unwrap_err();

    assert!(error.contains("正文过大"));
}

#[test]
fn export_cursor_pages_without_offset() {
    let (db, dir) = temp_db();
    let records = [
        make_record("first", "cursor-1", &[]),
        make_record("second", "cursor-2", &[]),
    ];
    db.import_records_with_merge(&records, 100, None).unwrap();

    let first = db.get_records_for_export_page(1, None).unwrap();
    let cursor = ExportCursor {
        is_pinned: first[0].is_pinned,
        updated_at: first[0].updated_at.clone(),
        id: first[0].id,
    };
    let second = db.get_records_for_export_page(1, Some(&cursor)).unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
    cleanup(dir);
}

#[test]
fn import_sanitize_recomputes_expiry_and_rechecks_sensitive() {
    let (db, dir) = temp_db();
    let mut rec = make_record("Your verification code: 123456", "sanitize-1", &[]);
    // Hostile/stale bundle: marks content non-sensitive and carries a past
    // expiry. With sanitization enabled neither may survive the import.
    rec.is_sensitive = false;
    rec.auto_expire_at = Some("2020-01-01T00:00:00Z".into());
    let policy = ImportSanitize {
        recheck_sensitive: true,
        sensitive_auto_expire_seconds: 600,
    };

    db.import_records_with_merge(&[rec], 100, Some(policy))
        .unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_sensitive, "detection must re-flag the record");
    let expiry = rows[0].auto_expire_at.as_deref().expect("expiry set");
    assert!(
        expiry > "2026-01-01T00:00:00Z",
        "expiry must be recomputed from now, got {expiry}"
    );
    cleanup(dir);
}

#[test]
fn import_sanitize_never_downgrades_sensitive_flag() {
    let (db, dir) = temp_db();
    let mut rec = make_record("plain text", "sanitize-2", &[]);
    rec.is_sensitive = true; // remote says sensitive even though text is plain
    let policy = ImportSanitize {
        recheck_sensitive: true,
        sensitive_auto_expire_seconds: 600,
    };

    db.import_records_with_merge(&[rec], 100, Some(policy))
        .unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_sensitive);
    assert!(rows[0].auto_expire_at.is_some());
    cleanup(dir);
}

#[test]
fn import_sanitize_preserves_past_expiry_when_disabled() {
    let (db, dir) = temp_db();
    let mut rec = make_record("plain text", "sanitize-3", &[]);
    rec.is_sensitive = true;
    rec.auto_expire_at = Some("2020-01-01T00:00:00Z".into());

    // Legacy callers (no policy) keep the previous passthrough behaviour.
    db.import_records_with_merge(&[rec], 100, None).unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(
        rows[0].auto_expire_at.as_deref(),
        Some("2020-01-01T00:00:00Z")
    );
    cleanup(dir);
}

#[test]
fn import_preserves_remote_device_origin() {
    let (db, dir) = temp_db();
    let mut rec = make_record("remote origin", "origin-remote", &[]);
    rec.source_device_id = "dev-remote".to_string();
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows[0].source_device_id, "dev-remote");
    cleanup(dir);
}

#[test]
fn import_merge_keeps_earlier_creator_as_origin() {
    let (db, dir) = temp_db();
    // Local record created later with a local origin.
    let mut local = make_record("same", "origin-merge", &[]);
    local.source_device_id = "dev-local".to_string();
    local.created_at = "2026-06-01T00:00:00Z".to_string();
    local.updated_at = "2026-06-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[local], 100, None).unwrap();

    // Same hash from another device, created earlier → origin flips to it.
    let mut remote = make_record("same", "origin-merge", &[]);
    remote.source_device_id = "dev-remote".to_string();
    remote.created_at = "2026-01-01T00:00:00Z".to_string();
    remote.updated_at = "2026-07-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[remote], 100, None).unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows[0].source_device_id, "dev-remote");
    cleanup(dir);
}

#[test]
fn import_merge_never_overwrites_known_origin() {
    let (db, dir) = temp_db();
    let mut local = make_record("same", "origin-keep", &[]);
    local.source_device_id = "dev-local".to_string();
    local.created_at = "2026-01-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[local], 100, None).unwrap();

    // Legacy incoming (empty origin) must not erase the known origin.
    let mut legacy = make_record("same", "origin-keep", &[]);
    legacy.created_at = "2025-01-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[legacy], 100, None).unwrap();

    // Newer incoming with a different origin must not overwrite the first one.
    let mut other = make_record("same", "origin-keep", &[]);
    other.source_device_id = "dev-other".to_string();
    other.created_at = "2026-06-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[other], 100, None).unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows[0].source_device_id, "dev-local");
    cleanup(dir);
}

#[test]
fn import_merge_fills_empty_origin_from_incoming() {
    let (db, dir) = temp_db();
    // Legacy local row (no origin).
    let mut local = make_record("same", "origin-fill", &[]);
    local.created_at = "2026-06-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[local], 100, None).unwrap();

    // Incoming with a known origin → the empty local slot is filled.
    let mut remote = make_record("same", "origin-fill", &[]);
    remote.source_device_id = "dev-remote".to_string();
    remote.created_at = "2026-01-01T00:00:00Z".to_string();
    db.import_records_with_merge(&[remote], 100, None).unwrap();

    let rows = db.get_records_for_export(10, 0).unwrap();
    assert_eq!(rows[0].source_device_id, "dev-remote");
    cleanup(dir);
}
