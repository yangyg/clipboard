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
