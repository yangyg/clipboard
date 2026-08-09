//! Tests for record insert/dedup/hash-uniqueness. Lives in its own file
//! (mirroring `schema_tests.rs`) to keep `records_write.rs` under the
//! 500-line cap.
use crate::db::{ClipboardDb, ContentType};
use crate::detect::{sha256_hash, sha256_hash_bytes};
use std::path::PathBuf;

fn temp_db() -> (ClipboardDb, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_records_write_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    (db, dir)
}

fn cleanup(dir: PathBuf) {
    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
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
    let err = db.conn.lock().execute(
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
