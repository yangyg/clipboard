//! Schema DDL, FTS5 management, and schema-version stamping.
use rusqlite::{Connection, Result as SqlResult};

use super::ClipboardDb;

/// Increment when adding tables, columns, or indexes that older DBs must migrate.
/// Stored in `settings(key='schema_version')` so doctor / diagnostics can verify
/// the on-disk schema matches what this binary expects.
const SCHEMA_VERSION: i64 = 1;

impl ClipboardDb {
    pub(super) fn ensure_fts(conn: &Connection) -> SqlResult<()> {
        // v2: FTS5 'delete' command fails with "SQL logic error" on some SQLite
        // builds (incl. Windows); use DELETE FROM fts WHERE rowid=... instead.
        // v3: FTS au only on content (dedup source updates must not rebuild FTS);
        //     tag→FTS refresh is application-driven (batch auto-tag once).
        const FTS_VERSION: &str = "4";
        let current: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'fts_version'",
                [],
                |row| row.get(0),
            )
            .ok();
        if current.as_deref() == Some(FTS_VERSION) {
            // Ensure table still exists (e.g. user deleted it manually)
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='records_fts'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if exists {
                return Ok(());
            }
        }

        conn.execute_batch(
            r#"
            DROP TRIGGER IF EXISTS records_fts_ai;
            DROP TRIGGER IF EXISTS records_fts_ad;
            DROP TRIGGER IF EXISTS records_fts_au;
            DROP TRIGGER IF EXISTS record_tags_fts_ai;
            DROP TRIGGER IF EXISTS record_tags_fts_ad;
            DROP TRIGGER IF EXISTS tags_fts_au;
            DROP TABLE IF EXISTS records_fts;
            "#,
        )?;

        // trigram: substring MATCH for clipboard-style search (needs ≥3 chars)
        conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE records_fts USING fts5(
                content,
                source_app,
                source_window,
                tags,
                alias,
                tokenize = 'trigram'
            );

            CREATE TRIGGER records_fts_ai AFTER INSERT ON records BEGIN
                INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                VALUES (
                    new.id,
                    new.content,
                    new.source_app,
                    new.source_window,
                    COALESCE((
                        SELECT group_concat(t.name, ' ')
                        FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE rt.record_id = new.id
                    ), ''),
                    new.alias
                );
            END;

            CREATE TRIGGER records_fts_ad AFTER DELETE ON records BEGIN
                DELETE FROM records_fts WHERE rowid = old.id;
            END;

            -- Only content changes rebuild FTS. Dedup updates of source_app/window
            -- must not rewrite the full content into FTS on every re-copy.
            -- Alias updates call refresh_record_fts from set_record_alias.
            CREATE TRIGGER records_fts_au AFTER UPDATE OF content ON records BEGIN
                DELETE FROM records_fts WHERE rowid = old.id;
                INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                VALUES (
                    new.id,
                    new.content,
                    new.source_app,
                    new.source_window,
                    COALESCE((
                        SELECT group_concat(t.name, ' ')
                        FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE rt.record_id = new.id
                    ), ''),
                    new.alias
                );
            END;

            CREATE TRIGGER tags_fts_au AFTER UPDATE OF name ON tags BEGIN
                DELETE FROM records_fts WHERE rowid IN (
                    SELECT rt.record_id FROM record_tags rt WHERE rt.tag_id = new.id
                );
                INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                SELECT
                    r.id,
                    r.content,
                    r.source_app,
                    r.source_window,
                    COALESCE((
                        SELECT group_concat(t.name, ' ')
                        FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE rt.record_id = r.id
                    ), ''),
                    r.alias
                FROM records r
                WHERE r.id IN (SELECT rt.record_id FROM record_tags rt WHERE rt.tag_id = new.id);
            END;
            "#,
        )?;

        conn.execute_batch(
            r#"
            INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
            SELECT
                r.id,
                r.content,
                r.source_app,
                r.source_window,
                COALESCE((
                    SELECT group_concat(t.name, ' ')
                    FROM record_tags rt
                    INNER JOIN tags t ON t.id = rt.tag_id
                    WHERE rt.record_id = r.id
                ), ''),
                r.alias
            FROM records r;
            "#,
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('fts_version', ?)",
            [FTS_VERSION],
        )?;
        Ok(())
    }

    /// Write `schema_version` into the settings table so external tools (doctor)
    /// and future migration gates can verify the on-disk schema.
    pub(super) fn apply_schema_version(conn: &Connection) -> SqlResult<()> {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('schema_version', ?)",
            [SCHEMA_VERSION.to_string().as_str()],
        )?;
        Ok(())
    }

    /// Read the schema version stored in the database. Returns `None` when the
    /// key is absent (database created before versioning was introduced).
    pub fn read_schema_version(conn: &Connection) -> Option<i64> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |row| {
                let s: String = row.get(0)?;
                Ok(s.parse::<i64>().unwrap_or(0))
            },
        )
        .ok()
    }

    pub fn schema_version() -> i64 {
        SCHEMA_VERSION
    }

    /// Rebuild one FTS row (tags / source) without per-tag triggers.
    pub(super) fn refresh_record_fts(conn: &Connection, record_id: i64) -> SqlResult<()> {
        conn.execute("DELETE FROM records_fts WHERE rowid = ?", [record_id])?;
        conn.execute(
            r#"
            INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
            SELECT
                r.id,
                r.content,
                r.source_app,
                r.source_window,
                COALESCE((
                    SELECT group_concat(t.name, ' ')
                    FROM record_tags rt
                    INNER JOIN tags t ON t.id = rt.tag_id
                    WHERE rt.record_id = r.id
                ), ''),
                r.alias
            FROM records r WHERE r.id = ?
            "#,
            [record_id],
        )?;
        Ok(())
    }

    /// Idempotent migrations for databases created before later columns /
    /// indexes existed. Columns are added only when `PRAGMA table_info` shows
    /// them missing, so a duplicate-column error can never occur and genuine
    /// failures are NOT swallowed (unlike the historical `ALTER … .ok()`).
    pub(super) fn migrate_schema(conn: &Connection) -> SqlResult<()> {
        let existing_cols: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(records)")?;
            let cols = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<SqlResult<std::collections::HashSet<String>>>()?;
            cols
        };
        const MIGRATE_COLUMNS: &[(&str, &str)] = &[
            ("is_trashed", "INTEGER NOT NULL DEFAULT 0"),
            ("media_path", "TEXT"),
            ("thumb_path", "TEXT"),
            ("width", "INTEGER"),
            ("height", "INTEGER"),
            ("content_html", "TEXT"),
            ("content_len", "INTEGER NOT NULL DEFAULT 0"),
            ("alias", "TEXT NOT NULL DEFAULT ''"),
        ];
        for (name, ddl) in MIGRATE_COLUMNS {
            if !existing_cols.contains(*name) {
                conn.execute_batch(&format!("ALTER TABLE records ADD COLUMN {name} {ddl}"))?;
            }
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_records_trashed_updated
             ON records(is_trashed, updated_at DESC);
             CREATE INDEX IF NOT EXISTS idx_records_trashed_pinned_updated
             ON records(is_trashed, is_pinned, updated_at DESC);
             CREATE INDEX IF NOT EXISTS idx_records_hash_active
             ON records(hash, is_trashed);",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;

    /// Helper: create a fresh in-memory DB and run the full schema init.
    fn fresh_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .unwrap();
        // Replicate the exact schema init from ClipboardDb::new (CREATE TABLE IF NOT EXISTS + migrations)
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS records (
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
                is_trashed INTEGER NOT NULL DEFAULT 0,
                auto_expire_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                media_path TEXT,
                thumb_path TEXT,
                width INTEGER,
                height INTEGER,
                content_html TEXT,
                content_len INTEGER NOT NULL DEFAULT 0,
                alias TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_records_updated_at ON records(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_hash ON records(hash);
            CREATE INDEX IF NOT EXISTS idx_records_content_type ON records(content_type);
            CREATE INDEX IF NOT EXISTS idx_records_is_favorite ON records(is_favorite);
            CREATE INDEX IF NOT EXISTS idx_records_trashed_updated
                ON records(is_trashed, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_trashed_pinned_updated
                ON records(is_trashed, is_pinned, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_hash_active
                ON records(hash, is_trashed);
            CREATE INDEX IF NOT EXISTS idx_records_auto_expire
                ON records(auto_expire_at) WHERE auto_expire_at IS NOT NULL;
            "#,
        )
        .unwrap();
        // Apply the real idempotent migration logic — columns already present
        // are skipped via PRAGMA table_info, so no duplicate-column errors.
        ClipboardDb::migrate_schema(&conn).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL DEFAULT '#6366f1',
                is_auto INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS record_tags (
                record_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (record_id, tag_id),
                FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_record_tags_tag_id ON record_tags(tag_id);
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn
    }

    /// Expected tables that must exist after schema init.
    const EXPECTED_TABLES: &[&str] = &["records", "tags", "record_tags", "settings"];

    /// Expected columns for the `records` table (column_name → must be queryable).
    const EXPECTED_RECORD_COLS: &[&str] = &[
        "id", "content", "content_type", "source_app", "source_window", "hash",
        "copy_count", "is_favorite", "is_pinned", "is_sensitive", "is_trashed",
        "auto_expire_at", "created_at", "updated_at", "media_path", "thumb_path",
        "width", "height", "content_html", "content_len", "alias",
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
        let mut stmt = conn
            .prepare("PRAGMA table_info(records)")
            .unwrap();
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
        // Stamp version the same way ClipboardDb::new does
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('schema_version', ?1)",
            [ClipboardDb::schema_version().to_string().as_str()],
        )
        .unwrap();
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
        use crate::db::{RECORD_COLS, RECORD_COLS_LIST};
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
}
