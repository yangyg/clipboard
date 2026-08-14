//! Tests for record insert/dedup/hash-uniqueness. Lives in its own file
//! (mirroring `schema_tests.rs`) to keep `records_write.rs` under the
//! 800-line cap.
use crate::db::{ClipboardDb, ContentType};
use crate::detect::{sha256_hash, sha256_hash_bytes};
use std::path::PathBuf;

fn temp_db() -> (ClipboardDb, PathBuf) {
    super::test_util::temp_db("records_write")
}

fn cleanup(dir: PathBuf) {
    super::test_util::cleanup(dir)
}

fn insert(db: &ClipboardDb, content: &str) -> (i64, bool, crate::ClipboardRecord) {
    // Mirror the capture pipeline's double-hash text format.
    let hash = sha256_hash(&sha256_hash(content));
    db.insert_record(
        content,
        &ContentType::Text,
        &hash,
        false,
        1000,
        600,
        "app.exe",
        "win",
        "",
        None,
        None,
    )
    .unwrap()
}

#[test]
fn record_hash_exists_matches_active_and_trashed_rows() {
    let (db, dir) = temp_db();
    let (id, is_new, _) = insert(&db, "hello world");
    assert!(is_new);
    let text_hash = sha256_hash(&sha256_hash("hello world"));
    assert!(db.record_hash_exists(&text_hash).unwrap());
    assert!(!db
        .record_hash_exists(&sha256_hash(&sha256_hash("absent")))
        .unwrap());

    // Trashed rows still count as "already exists" for history re-import:
    // importing must not resurrect trashed items into the active list, even
    // though the partial unique index alone would allow a fresh insert.
    db.trash_record(id).unwrap();
    assert!(db.record_hash_exists(&text_hash).unwrap());
    cleanup(dir);
}

#[test]
fn record_hash_exists_false_for_image_hash_and_empty_db() {
    let (db, dir) = temp_db();
    let image_hash = sha256_hash_bytes(&[1u8, 2, 3, 4]);
    assert!(!db.record_hash_exists(&image_hash).unwrap());
    cleanup(dir);
}

#[test]
fn unique_active_hash_blocks_duplicate_insert() {
    let (db, dir) = temp_db();
    let hash = sha256_hash(&sha256_hash("dup guard"));
    insert(&db, "dup guard");
    // The DB itself must reject a second active row with the same hash —
    // dedup must not rely solely on insert_record's application-level probe.
    let err = db.lock_write().execute(
        "INSERT INTO records (content, content_type, hash, created_at, updated_at)
         VALUES ('dup guard', 'text', ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [&hash],
    );
    assert!(matches!(
        err,
        Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        ))
    ));
    cleanup(dir);
}

#[test]
fn recopy_after_trash_inserts_fresh_and_restore_prefers_active() {
    let (db, dir) = temp_db();
    let (first_id, _, _) = insert(&db, "lifecycle");
    db.trash_record(first_id).unwrap();

    // Re-copying a trashed item must insert a fresh active record.
    let (second_id, is_new, _) = insert(&db, "lifecycle");
    assert!(is_new);
    assert_ne!(first_id, second_id);

    // Restoring the stale trashed row must not violate the unique index;
    // the active copy wins and the trashed row is dropped.
    db.restore_record(first_id).unwrap();
    assert!(db.get_record(first_id).unwrap().is_none());
    assert!(db.get_record(second_id).unwrap().is_some());
    cleanup(dir);
}

#[test]
fn insert_stamps_device_origin_and_recopy_keeps_it() {
    let (db, dir) = temp_db();
    let settings = crate::Settings {
        webdav_device_id: "dev-1".to_string(),
        webdav_device_name: "办公电脑".to_string(),
        ..crate::Settings::default()
    };
    db.save_settings(&settings).unwrap();

    let (_, is_new, rec) = insert(&db, "origin text");
    assert!(is_new);
    assert_eq!(rec.source_device_id, "dev-1");

    // Re-copying the same content (dedup refresh) must not re-label the origin.
    let (_, is_new2, rec2) = insert(&db, "origin text");
    assert!(!is_new2);
    assert_eq!(rec2.source_device_id, "dev-1");
    cleanup(dir);
}

fn insert_html(
    db: &ClipboardDb,
    content: &str,
    html: Option<&str>,
) -> (i64, bool, crate::ClipboardRecord) {
    let hash = sha256_hash(&sha256_hash(content));
    db.insert_record(
        content,
        &ContentType::Text,
        &hash,
        false,
        1000,
        600,
        "app.exe",
        "win",
        "",
        None,
        html,
    )
    .unwrap()
}

#[test]
fn recopy_refreshes_content_html_when_payload_changes() {
    let (db, dir) = temp_db();
    let (id, is_new, _) = insert_html(&db, "hello", Some("<b>hello</b>"));
    assert!(is_new);

    let (_, is_new2, _) = insert_html(&db, "hello", Some("<i>hello</i>"));
    assert!(!is_new2);
    let rec = db.get_record(id).unwrap().unwrap();
    assert_eq!(rec.content_html.as_deref(), Some("<i>hello</i>"));
    cleanup(dir);
}

#[test]
fn recopy_without_html_keeps_existing_rich_text() {
    let (db, dir) = temp_db();
    let (id, _, _) = insert_html(&db, "hello", Some("<b>hello</b>"));
    let (_, is_new, _) = insert(&db, "hello");
    assert!(!is_new);
    let rec = db.get_record(id).unwrap().unwrap();
    assert_eq!(rec.content_html.as_deref(), Some("<b>hello</b>"));
    cleanup(dir);
}

#[test]
fn take_record_for_paste_does_not_increment_copy_count() {
    let (db, dir) = temp_db();
    let (id, _, rec) = insert(&db, "paste me");
    assert_eq!(rec.copy_count, 0);
    let loaded = db.take_record_for_paste(id).unwrap().unwrap();
    assert_eq!(loaded.copy_count, 0);
    assert_eq!(db.get_record(id).unwrap().unwrap().copy_count, 0);
    cleanup(dir);
}

#[test]
fn bump_copy_count_increments_active_row_only() {
    let (db, dir) = temp_db();
    let (id, _, _) = insert(&db, "paste me");
    db.bump_copy_count(id).unwrap();
    assert_eq!(db.get_record(id).unwrap().unwrap().copy_count, 1);
    db.trash_record(id).unwrap();
    db.bump_copy_count(id).unwrap();
    // Trashed rows are not paste targets — count stays at the last active bump.
    assert_eq!(db.get_record(id).unwrap().unwrap().copy_count, 1);
    cleanup(dir);
}

#[test]
fn insert_record_applies_auto_tags_on_new_insert() {
    let (db, dir) = temp_db();
    let content = "https://example.com/auto-tag";
    let hash = sha256_hash(&sha256_hash(content));
    let (id, is_new, rec) = db
        .insert_record(
            content,
            &ContentType::Link,
            &hash,
            false,
            1000,
            600,
            "app.exe",
            "win",
            "",
            None,
            None,
        )
        .unwrap();
    assert!(is_new);
    assert!(
        rec.tags.iter().any(|t| t == "链接"),
        "fresh insert must attach auto-tags in the same write lock: {:?}",
        rec.tags
    );
    assert_eq!(db.get_record_tag_names(id).unwrap(), rec.tags);

    // Hash-dedup refresh must not retag.
    let (id2, is_new2, _) = db
        .insert_record(
            content,
            &ContentType::Link,
            &hash,
            false,
            1000,
            600,
            "other.exe",
            "win2",
            "",
            None,
            None,
        )
        .unwrap();
    assert!(!is_new2);
    assert_eq!(id2, id);
    cleanup(dir);
}
