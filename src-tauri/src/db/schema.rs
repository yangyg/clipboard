//! Schema DDL, FTS5 management, and schema-version stamping.
use rusqlite::{Connection, Result as SqlResult};

use super::ClipboardDb;

/// Increment when adding tables, columns, or indexes that older DBs must migrate.
/// Stored in `settings(key='schema_version')` so doctor / diagnostics can verify
/// the on-disk schema matches what this binary expects.
const SCHEMA_VERSION: i64 = 8;

/// Default tag definitions seeded on schema init and re-seeded after
/// `clear_all_data` so a fresh slate still ships the built-in tags.
/// Shared const — never drift between init and clear-all.
pub(super) const DEFAULT_TAGS_INSERT: &str =
    "INSERT OR IGNORE INTO tags (name, color, is_auto) VALUES
    ('部署', '#22c55e', 1),
    ('前端', '#6366f1', 1),
    ('链接', '#eab308', 1),
    ('重要', '#ef4444', 0),
    ('设计', '#a855f7', 0);";

/// FTS indexes only the first N chars of `content`. Trigram index size grows
/// ~3-5x the source text, so a 10MB-cap record would otherwise build a ~30MB
/// FTS entry and stall the capture write lock. Truncation keeps writes bounded;
/// searches beyond the prefix fall back to the short-query `instr` path.
const FTS_CONTENT_MAX_CHARS: i64 = 32 * 1024;

impl ClipboardDb {
    pub(super) fn initialize_schema(conn: &Connection) -> SqlResult<()> {
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
                alias TEXT NOT NULL DEFAULT '',
                source_name TEXT NOT NULL DEFAULT '',
                source_device_id TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_records_updated_at ON records(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_hash ON records(hash);
            CREATE INDEX IF NOT EXISTS idx_records_content_type ON records(content_type);
            CREATE INDEX IF NOT EXISTS idx_records_is_favorite ON records(is_favorite);
            CREATE INDEX IF NOT EXISTS idx_records_hash_active
                ON records(hash, is_trashed);
            CREATE INDEX IF NOT EXISTS idx_records_auto_expire
                ON records(auto_expire_at) WHERE auto_expire_at IS NOT NULL;
            "#,
        )?;
        Self::migrate_schema(conn)?;
        conn.execute_batch(&format!(
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
            CREATE TABLE IF NOT EXISTS sync_tombstones (
                hash TEXT PRIMARY KEY,
                deleted_at TEXT NOT NULL,
                is_sensitive INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS search_history (
                query TEXT PRIMARY KEY,
                search_count INTEGER NOT NULL DEFAULT 1,
                last_searched_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                synced_at TEXT NOT NULL,
                action TEXT NOT NULL,
                success INTEGER NOT NULL,
                pulled INTEGER NOT NULL DEFAULT 0,
                pushed INTEGER NOT NULL DEFAULT 0,
                merged INTEGER NOT NULL DEFAULT 0,
                tags_pulled INTEGER NOT NULL DEFAULT 0,
                tags_pushed INTEGER NOT NULL DEFAULT 0,
                media_downloaded INTEGER NOT NULL DEFAULT 0,
                media_uploaded INTEGER NOT NULL DEFAULT 0,
                media_skipped INTEGER NOT NULL DEFAULT 0,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_sync_history_synced_at
                ON sync_history(synced_at DESC);

            {DEFAULT_TAGS_INSERT}
            "#,
            DEFAULT_TAGS_INSERT = DEFAULT_TAGS_INSERT,
        ))?;
        // v7: keyset pagination orders by (is_pinned, updated_at, id) with an id
        // tiebreak, so the composite indexes must carry id as the last column.
        // Runs here (after the `settings` table exists) so the one-shot flag has
        // somewhere to live.
        Self::migrate_keyset_indexes(conn)?;
        // v8: the default list ORDER BY is `is_pinned DESC, updated_at DESC,
        // id DESC`; v2's index stored is_pinned ascending, so the planner could
        // not serve the ORDER BY in a single direction and fell back to a full
        // temp B-tree sort (tens of ms at 50k+ rows). Store is_pinned
        // descending so page-1 and keyset queries stop after LIMIT rows.
        Self::migrate_list_order_index(conn)?;
        // Runs AFTER the `settings` table exists (its one-shot gate lives there).
        Self::enforce_active_hash_uniqueness(conn)?;
        Ok(())
    }

    /// One-shot (`list_order_index_v3`): rebuild the main list index with the
    /// pinned column stored descending so the default ORDER BY
    /// `is_pinned DESC, updated_at DESC, id DESC` matches the index exactly
    /// (equality on `is_trashed`). The v2 shape could only match is_pinned in
    /// the ascending direction, forcing `USE TEMP B-TREE FOR LAST TERM OF
    /// ORDER BY` on every first page.
    fn migrate_list_order_index(conn: &Connection) -> SqlResult<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'list_order_index_v3'",
                [],
                |row| row.get(0),
            )
            .ok();
        if done.as_deref() == Some("1") {
            return Ok(());
        }
        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_records_trashed_pinned_updated;
             CREATE INDEX idx_records_trashed_pinned_updated
                 ON records(is_trashed, is_pinned DESC, updated_at DESC, id DESC);
             INSERT OR REPLACE INTO settings (key, value) VALUES ('list_order_index_v3', '1');",
        )?;
        Ok(())
    }

    /// One-shot (settings flag `keyset_index_v2`): widen the two list indexes
    /// with the `id` tiebreak column so keyset predicates
    /// (`updated_at = ? AND id < ?`) and `ORDER BY … id DESC` can use the index
    /// without a separate sort pass. Old shapes are dropped by name first.
    fn migrate_keyset_indexes(conn: &Connection) -> SqlResult<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'keyset_index_v2'",
                [],
                |row| row.get(0),
            )
            .ok();
        if done.as_deref() == Some("1") {
            return Ok(());
        }
        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_records_trashed_updated;
             DROP INDEX IF EXISTS idx_records_trashed_pinned_updated;
             CREATE INDEX idx_records_trashed_updated
                 ON records(is_trashed, updated_at DESC, id DESC);
             CREATE INDEX idx_records_trashed_pinned_updated
                 ON records(is_trashed, is_pinned, updated_at DESC, id DESC);
             INSERT OR REPLACE INTO settings (key, value) VALUES ('keyset_index_v2', '1');",
        )?;
        Ok(())
    }

    /// Move the active-hash dedup invariant from application logic into the
    /// database: clean pre-existing duplicate active rows, then install a
    /// partial unique index. Partial (active-only) uniqueness: trashed rows
    /// must NOT hold the hash slot, otherwise re-copying a trashed item
    /// violates the constraint instead of inserting a fresh record.
    fn enforce_active_hash_uniqueness(conn: &Connection) -> SqlResult<()> {
        // Legacy DBs (older/dev builds) may still carry a full
        // `UNIQUE(hash)` index (`idx_records_hash_unique`). A full unique
        // index forbids the active+trashed coexistence the partial index is
        // designed to allow — re-copying a trashed item and WebDAV re-insert
        // after a tombstone pull both fail with
        // "UNIQUE constraint failed: records.hash". Drop it; lookups keep
        // using the non-unique `idx_records_hash`.
        conn.execute_batch("DROP INDEX IF EXISTS idx_records_hash_unique;")?;
        Self::dedupe_active_hashes(conn)?;
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_records_hash_active
             ON records(hash) WHERE is_trashed = 0;",
        )?;
        Ok(())
    }

    pub(super) fn ensure_fts(conn: &Connection) -> SqlResult<()> {
        // v2: FTS5 'delete' command fails with "SQL logic error" on some SQLite
        // builds (incl. Windows); use DELETE FROM fts WHERE rowid=... instead.
        // v3: FTS au only on content (dedup source updates must not rebuild FTS);
        //     tag→FTS refresh is application-driven (batch auto-tag once).
        const FTS_VERSION: &str = "5";
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
                    substr(new.content, 1, 32768),
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
                    substr(new.content, 1, 32768),
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
                    substr(r.content, 1, 32768),
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
                substr(r.content, 1, 32768),
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

    pub fn schema_version() -> i64 {
        SCHEMA_VERSION
    }

    /// Rebuild one FTS row (tags / source) without per-tag triggers.
    pub(super) fn refresh_record_fts(conn: &Connection, record_id: i64) -> SqlResult<()> {
        conn.execute("DELETE FROM records_fts WHERE rowid = ?", [record_id])?;
        conn.execute(
            &format!(
                "INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                 SELECT
                    r.id,
                    {},
                    r.source_app,
                    r.source_window,
                    COALESCE((
                        SELECT group_concat(t.name, ' ')
                        FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE rt.record_id = r.id
                    ), ''),
                    r.alias
                 FROM records r WHERE r.id = ?",
                Self::fts_content_sql()
            ),
            [record_id],
        )?;
        Ok(())
    }

    /// Rebuild FTS rows for many records in two statements (delete + insert),
    /// replacing the per-record `refresh_record_fts` N+1 pattern on bulk paths
    /// (tag deletion, import merges).
    pub(super) fn refresh_records_fts_batch(
        conn: &Connection,
        record_ids: &[i64],
    ) -> SqlResult<()> {
        if record_ids.is_empty() {
            return Ok(());
        }
        let placeholders = Self::id_placeholders(record_ids.len());
        let params: Vec<&dyn rusqlite::types::ToSql> = record_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        conn.execute(
            &format!("DELETE FROM records_fts WHERE rowid IN ({placeholders})"),
            params.as_slice(),
        )?;
        conn.execute(
            &format!(
                "INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                 SELECT
                    r.id,
                    {},
                    r.source_app,
                    r.source_window,
                    COALESCE((
                        SELECT group_concat(t.name, ' ')
                        FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE rt.record_id = r.id
                    ), ''),
                    r.alias
                 FROM records r WHERE r.id IN ({placeholders})",
                Self::fts_content_sql()
            ),
            params.as_slice(),
        )?;
        Ok(())
    }

    /// SQL expression for the FTS `content` column (truncated to keep the
    /// trigram index bounded). Single source of truth for triggers / backfill /
    /// manual rebuilds. Keep in sync with FTS_CONTENT_MAX_CHARS (raw trigger
    /// SQL cannot interpolate the const, so the literal appears in the DDL too).
    pub(super) fn fts_content_sql() -> String {
        format!("substr(content, 1, {})", FTS_CONTENT_MAX_CHARS)
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
            ("source_name", "TEXT NOT NULL DEFAULT ''"),
            ("source_device_id", "TEXT NOT NULL DEFAULT ''"),
        ];
        for (name, ddl) in MIGRATE_COLUMNS {
            if !existing_cols.contains(*name) {
                conn.execute_batch(&format!("ALTER TABLE records ADD COLUMN {name} {ddl}"))?;
            }
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_records_hash_active
             ON records(hash, is_trashed);",
        )?;
        Ok(())
    }

    /// One-shot: drop duplicate **active** hash rows (keep the most recently
    /// updated) so `uq_records_hash_active` can be created. Capture-side dedup
    /// is application logic and has historically raced; this migration moves
    /// the invariant into the database itself. Trashed duplicates are left
    /// alone — they sit outside the partial index and a trash sweep deletes
    /// them eventually.
    fn dedupe_active_hashes(conn: &Connection) -> SqlResult<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'hash_unique_v1'",
                [],
                |row| row.get(0),
            )
            .ok();
        if done.as_deref() == Some("1") {
            return Ok(());
        }
        conn.execute(
            "DELETE FROM records WHERE is_trashed = 0 AND id NOT IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (
                        PARTITION BY hash
                        ORDER BY updated_at DESC, id DESC
                    ) AS rn
                    FROM records
                    WHERE is_trashed = 0
                )
                WHERE rn = 1
             )",
            [],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('hash_unique_v1', '1')",
            [],
        )?;
        Ok(())
    }
}
