//! Schema compatibility tests (test-only module, kept separate from schema.rs
//! so the production file stays under the 500-line cap).

use crate::db::{ClipboardDb, ContentType, RECORD_COLS, RECORD_COLS_LIST};

/// Helper: create a fresh in-memory DB and run the full schema init.
fn fresh_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .unwrap();
    ClipboardDb::initialize_schema(&conn).unwrap();
    conn
}

#[test]
fn text_hash_v2_rehashes_and_merges_html_variant_duplicates() {
    let conn = fresh_db();
    ClipboardDb::ensure_fts(&conn).unwrap();
    // Simulate legacy rows: identical content, different hashes (the old
    // scheme baked CF_HTML bytes into the fingerprint).
    conn.execute(
        "INSERT INTO records (content, content_type, hash, copy_count, is_favorite, created_at, updated_at)
         VALUES ('hello world', 'text', 'legacy-a', 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO records (content, content_type, hash, copy_count, created_at, updated_at)
         VALUES ('hello world', 'text', 'legacy-b', 2, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
        [],
    )
    .unwrap();
    // Tag on the loser (older updated_at) must survive the merge.
    conn.execute(
        "INSERT INTO tags (name, color) VALUES ('work', '#ef4444')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO record_tags (record_id, tag_id) VALUES (1, 1)",
        [],
    )
    .unwrap();

    ClipboardDb::migrate_text_hash_v2(&conn).unwrap();

    let expected_hash = crate::detect::sha256_hash(&crate::detect::sha256_hash("hello world"));
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    // Winner is id=2 (newer updated_at); favorite OR'd, copy_count summed.
    let (hash, fav, copies): (String, i32, i32) = conn
        .query_row(
            "SELECT hash, is_favorite, copy_count FROM records",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(hash, expected_hash);
    assert_eq!(fav, 1);
    assert_eq!(copies, 3);
    let tag_links: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_tags WHERE record_id = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tag_links, 1);
    // FTS keeps exactly one row for the surviving record.
    let fts: i64 = conn
        .query_row("SELECT COUNT(*) FROM records_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fts, 1);

    // Idempotent: the settings flag short-circuits a second run.
    ClipboardDb::migrate_text_hash_v2(&conn).unwrap();
    let count2: i64 = conn
        .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count2, 1);
}

/// Expected tables that must exist after schema init.
const EXPECTED_TABLES: &[&str] = &[
    "records",
    "tags",
    "record_tags",
    "settings",
    "sync_tombstones",
    "search_history",
    "sync_history",
];

/// Expected columns for the `records` table (column_name → must be queryable).
const EXPECTED_RECORD_COLS: &[&str] = &[
    "id",
    "content",
    "content_type",
    "source_app",
    "source_window",
    "hash",
    "copy_count",
    "is_favorite",
    "is_pinned",
    "is_sensitive",
    "is_trashed",
    "auto_expire_at",
    "created_at",
    "updated_at",
    "media_path",
    "thumb_path",
    "width",
    "height",
    "content_html",
    "content_len",
    "alias",
    "source_name",
    "source_device_id",
];

/// Expected indexes (name → must exist in sqlite_master).
const EXPECTED_INDEXES: &[&str] = &[
    "idx_records_updated_at",
    "idx_records_hash",
    "idx_records_content_type",
    "idx_records_is_favorite",
    "idx_records_trashed_updated",
    "idx_records_trashed_pinned_updated",
    "idx_records_hash_active",
    "idx_records_auto_expire",
    "idx_record_tags_tag_id",
    "idx_sync_history_synced_at",
];

#[test]
fn schema_all_expected_tables_exist() {
    let conn = fresh_db();
    for table in EXPECTED_TABLES {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "Missing table: {table}");
    }
}

#[test]
fn schema_records_has_all_columns() {
    let conn = fresh_db();
    // PRAGMA table_info returns (cid, name, type, notnull, dflt_value, pk)
    let mut stmt = conn.prepare("PRAGMA table_info(records)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in EXPECTED_RECORD_COLS {
        assert!(
            cols.iter().any(|c| c == expected),
            "Missing column '{expected}' in records table. Found: {cols:?}"
        );
    }
}

#[test]
fn schema_all_expected_indexes_exist() {
    let conn = fresh_db();
    for idx in EXPECTED_INDEXES {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1",
                [idx],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "Missing index: {idx}");
    }
}

#[test]
fn schema_version_is_stamped_after_init() {
    let conn = fresh_db();
    // Stamp through the real idempotent path (not a manual INSERT) so the test
    // exercises the exact code ClipboardDb::new runs.
    ClipboardDb::apply_schema_version(&conn).unwrap();
    let stored: i64 = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |row| {
                let s: String = row.get(0)?;
                Ok(s.parse::<i64>().unwrap_or(0))
            },
        )
        .unwrap();
    assert_eq!(stored, ClipboardDb::schema_version());
}

/// Simulate an "old database" missing later-added columns, then verify
/// that the idempotent ALTER TABLE migrations bring it up to date.
#[test]
fn schema_migration_from_old_db_adds_missing_columns() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    // Create a "v0" records table WITHOUT the later-added columns
    conn.execute_batch(
        r#"
        CREATE TABLE records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            content_type TEXT NOT NULL DEFAULT 'text',
            source_app TEXT NOT NULL DEFAULT '',
            source_window TEXT NOT NULL DEFAULT '',
            hash TEXT NOT NULL,
            copy_count INTEGER NOT NULL DEFAULT 0,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            is_sensitive INTEGER NOT NULL DEFAULT 0,
            auto_expire_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )
    .unwrap();

    // Apply the real idempotent migration logic (adds only missing columns).
    ClipboardDb::migrate_schema(&conn).unwrap();

    // Verify all expected columns now exist
    let mut stmt = conn.prepare("PRAGMA table_info(records)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in EXPECTED_RECORD_COLS {
        assert!(
            cols.iter().any(|c| c == expected),
            "Migration failed: column '{expected}' missing after ALTER TABLE. Found: {cols:?}"
        );
    }
}

/// RECORD_COLS and RECORD_COLS_LIST must reference the same number of columns
/// (a mismatch causes silent row mapping bugs).
#[test]
fn schema_record_col_constants_have_same_arity() {
    let conn = fresh_db();
    // Count top-level commas (skip commas inside parentheses like substr(…))
    let count_top_level_commas = |s: &str| -> usize {
        let mut depth = 0i32;
        let mut count = 0usize;
        for ch in s.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => count += 1,
                _ => {}
            }
        }
        count
    };
    let full_arity = count_top_level_commas(RECORD_COLS) + 1;
    let list_arity = count_top_level_commas(RECORD_COLS_LIST) + 1;
    assert_eq!(
        full_arity, list_arity,
        "RECORD_COLS has {full_arity} columns but RECORD_COLS_LIST has {list_arity}; \
         they must match 1:1 for map_record_row to work"
    );
    // Also verify the DB actually has at least this many columns
    let mut stmt = conn.prepare("PRAGMA table_info(records)").unwrap();
    let col_count = stmt
        .query_map([], |_| Ok(()))
        .unwrap()
        .filter_map(|r| r.ok())
        .count();
    assert!(
        col_count >= full_arity,
        "records table has {col_count} columns but RECORD_COLS expects {full_arity}"
    );
}

#[test]
fn dedup_recopy_refreshes_fts_source_columns() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_fts_dedup_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    let hash = crate::detect::sha256_hash(&crate::detect::sha256_hash("same text"));

    let (id, is_new, _) = db
        .insert_record(
            "same text",
            &ContentType::Text,
            &hash,
            false,
            100,
            600,
            "first.exe",
            "First",
            "First",
            None,
            None,
        )
        .unwrap();
    assert!(is_new);

    // Re-copy from a different app must refresh the FTS source columns even
    // though the content (and therefore the content-only trigger) is unchanged.
    let (_, is_new2, _) = db
        .insert_record(
            "same text",
            &ContentType::Text,
            &hash,
            false,
            100,
            600,
            "second.exe",
            "Second",
            "Second",
            None,
            None,
        )
        .unwrap();
    assert!(!is_new2);

    let conn = db.conn.lock();
    let (app, win): (String, String) = conn
        .query_row(
            "SELECT source_app, source_window FROM records_fts WHERE rowid = ?",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(app, "second.exe");
    assert_eq!(win, "Second");
    drop(conn);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn map_record_row_binds_column_order_for_both_column_lists() {
    use crate::ClipboardRecord;

    let dir = std::env::temp_dir().join(format!(
        "clipvault_map_row_test_{}_{}",
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
        content: "hello <b>world</b>".into(),
        content_type: "text".into(),
        source_app: "app.exe".into(),
        source_window: "Window".into(),
        source_name: "Friendly".into(),
        source_device_id: "dev-origin".into(),
        hash: "map-row-hash".into(),
        copy_count: 3,
        is_favorite: true,
        is_pinned: false,
        is_sensitive: false,
        is_trashed: false,
        auto_expire_at: None,
        created_at: now.clone(),
        updated_at: now,
        tags: vec!["重要".into()],
        content_html: Some("<b>world</b>".into()),
        media_path: None,
        thumb_path: None,
        width: None,
        height: None,
        media_abs: None,
        thumb_abs: None,
        content_len: None,
        alias: "my-alias".into(),
    };
    db.import_records_with_merge(&[rec], 100, None).unwrap();
    let id = db.get_records_for_export(10, 0).unwrap()[0].id;

    let conn = db.conn.lock();
    for cols in [RECORD_COLS, RECORD_COLS_LIST] {
        let mut stmt = conn
            .prepare(&format!("SELECT {cols} FROM records WHERE id = ?"))
            .unwrap();
        let mut rows = stmt.query([id]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let mapped = db.map_record_row(row).unwrap();
        assert_eq!(mapped.content, "hello <b>world</b>");
        assert_eq!(mapped.source_app, "app.exe");
        assert_eq!(mapped.source_window, "Window");
        assert_eq!(mapped.source_name, "Friendly");
        assert_eq!(mapped.source_device_id, "dev-origin");
        // Import re-derives text hashes (sha256(sha256(content))), matching the
        // capture identity scheme — assert the stored value, not the input.
        let expected_hash =
            crate::detect::sha256_hash(&crate::detect::sha256_hash("hello <b>world</b>"));
        assert_eq!(mapped.hash, expected_hash);
        assert_eq!(mapped.copy_count, 3);
        assert!(mapped.is_favorite);
        assert_eq!(mapped.alias, "my-alias");
        assert_eq!(mapped.content_len, Some(18));
        if cols == RECORD_COLS {
            assert_eq!(mapped.content_html.as_deref(), Some("<b>world</b>"));
        } else {
            assert!(mapped.content_html.is_none(), "list rows must omit HTML");
        }
    }
    drop(conn);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn startup_drops_legacy_full_unique_hash_index() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_legacy_hash_unique_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Simulate a DB from a build that carried the historical FULL unique
    // `records(hash)` index alongside the current partial one (found in the
    // wild): the full index keeps holding the slot of a record after it is
    // trashed, so a fresh active insert with the same hash — local re-copy or
    // WebDAV pull re-insert — fails with
    // "UNIQUE constraint failed: records.hash". The fixture row is typed
    // `image` so the startup text-hash migration does not rewrite its hash.
    let conn = rusqlite::Connection::open(dir.join("test.db")).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .unwrap();
    ClipboardDb::initialize_schema(&conn).unwrap();
    conn.execute_batch(
        "CREATE UNIQUE INDEX idx_records_hash_unique ON records(hash);
         INSERT INTO records (content, content_type, hash, is_trashed, created_at, updated_at)
         VALUES ('legacy', 'image', 'h', 0, '2099-08-01T00:00:00Z', '2099-08-01T00:00:00Z');
         UPDATE records SET is_trashed = 1 WHERE id = 1;",
    )
    .unwrap();
    drop(conn);

    // Startup must drop the stale full-unique index (keeping the partial one).
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    let conn = db.conn.lock();
    let (legacy_gone, partial_kept): (i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_records_hash_unique'),
                (SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='uq_records_hash_active')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(legacy_gone, 0, "legacy full-unique index must be dropped");
    assert_eq!(partial_kept, 1, "partial active-unique index must survive");
    // The trashed row still holds the hash; a fresh active row with the same
    // hash must now be allowed (the WebDAV pull / re-copy path).
    conn.execute(
        "INSERT INTO records (content, content_type, hash, is_trashed, created_at, updated_at)
         VALUES ('fresh', 'image', 'h', 0, '2099-08-02T00:00:00Z', '2099-08-02T00:00:00Z')",
        [],
    )
    .unwrap();
    let (active, trashed): (i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM records WHERE hash = 'h' AND is_trashed = 0),
                (SELECT COUNT(*) FROM records WHERE hash = 'h' AND is_trashed = 1)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((active, trashed), (1, 1));
    drop(conn);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_succeeds_when_legacy_db_has_no_fts() {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_legacy_no_fts_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Simulate a pre-FTS database: current tables/columns, but the FTS virtual
    // table, its triggers and the fts_version stamp are absent.
    let conn = rusqlite::Connection::open(dir.join("test.db")).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .unwrap();
    ClipboardDb::initialize_schema(&conn).unwrap();
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS records_fts_ai;
         DROP TRIGGER IF EXISTS records_fts_ad;
         DROP TRIGGER IF EXISTS records_fts_au;
         DROP TRIGGER IF EXISTS record_tags_fts_ai;
         DROP TRIGGER IF EXISTS record_tags_fts_ad;
         DROP TRIGGER IF EXISTS tags_fts_au;
         DROP TABLE IF EXISTS records_fts;",
    )
    .unwrap();
    conn.execute("DELETE FROM settings WHERE key = 'fts_version'", [])
        .unwrap();
    // Seed duplicate text hashes so migrate_text_hash_v2 exercises its merge
    // path (refresh_record_fts) during startup.
    conn.execute(
        "INSERT INTO records (content, content_type, hash, created_at, updated_at)
         VALUES ('dup text', 'text', 'legacy-x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO records (content, content_type, hash, created_at, updated_at)
         VALUES ('dup text', 'text', 'legacy-y', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    // Startup must succeed and rebuild FTS before the hash-merge migration runs.
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    assert_eq!(db.get_records_for_export(10, 0).unwrap().len(), 1);
    let conn = db.conn.lock();
    let fts_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM records_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fts_rows, 1);
    drop(conn);

    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}
