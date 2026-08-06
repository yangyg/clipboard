//! Schema compatibility tests (test-only module, kept separate from schema.rs
//! so the production file stays under the 500-line cap).

use crate::db::{ClipboardDb, RECORD_COLS, RECORD_COLS_LIST};

/// Helper: create a fresh in-memory DB and run the full schema init.
fn fresh_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .unwrap();
    ClipboardDb::initialize_schema(&conn).unwrap();
    conn
}

/// Expected tables that must exist after schema init.
const EXPECTED_TABLES: &[&str] = &[
    "records",
    "tags",
    "record_tags",
    "settings",
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
