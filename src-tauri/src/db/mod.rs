use rusqlite::{Connection, Result as SqlResult};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fmt;
use crate::media;
use crate::Settings;

mod records;
mod schema;
mod settings;
mod stats;
mod tags;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Code,
    Link,
    Image,
    File,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentType::Text => write!(f, "text"),
            ContentType::Code => write!(f, "code"),
            ContentType::Link => write!(f, "link"),
            ContentType::Image => write!(f, "image"),
            ContentType::File => write!(f, "file"),
        }
    }
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Code => "code",
            ContentType::Link => "link",
            ContentType::Image => "image",
            ContentType::File => "file",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "text" => ContentType::Text,
            "code" => ContentType::Code,
            "link" => ContentType::Link,
            "image" => ContentType::Image,
            "file" => ContentType::File,
            _ => ContentType::Text,
        }
    }
}

/// Optional image metadata when inserting an image record.
#[derive(Debug, Clone)]
pub struct ImageMeta {
    pub media_path: String,
    pub thumb_path: String,
    pub width: i32,
    pub height: i32,
}

/// Full row including rich HTML (detail / paste / export).
const RECORD_COLS: &str = "id, content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, content_html,
               content_len, alias";

/// List/search: omit HTML, truncate content for IPC/memory; prefer content_len column.
const RECORD_COLS_LIST: &str = "id, substr(content, 1, 400) as content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, NULL as content_html,
               content_len, alias";

const ALIAS_MAX_CHARS: usize = 80;

pub struct ClipboardDb {
    /// Writer connection (schema, inserts, updates, deletes).
    conn: Mutex<Connection>,
    /// Small pool of query_only readers — WAL allows concurrent reads with the writer
    /// and with each other (unlike a single Mutex around one read conn).
    read_conns: Vec<Mutex<Connection>>,
    read_rr: std::sync::atomic::AtomicUsize,
    media_root: PathBuf,
    /// M-3: Cached string prefix (with trailing separator) for fast path enrichment.
    /// Avoids PathBuf alloc + to_string_lossy per record row.
    media_root_prefix: String,
    /// In-memory copy of `app_settings`, populated lazily. Avoids re-parsing the
    /// settings JSON on every clipboard event (the monitor reads it 2-3x/event).
    /// Arc allows cheap clone on the capture hot path (atomic refcount bump only).
    settings_cache: RwLock<Option<Arc<Settings>>>,
}

const READ_POOL_SIZE: usize = 3;

impl ClipboardDb {
    fn configure_connection(conn: &Connection, query_only: bool) -> SqlResult<()> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )?;
        if query_only {
            // Fail loudly if a "read" path accidentally tries to mutate.
            conn.execute_batch("PRAGMA query_only=ON;")?;
        }
        Ok(())
    }

    pub fn new(db_path: &Path, media_root: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        Self::configure_connection(&conn, false)?;

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
                ON records(auto_expire_at) WHERE auto_expire_at IS NOT NULL;"#,
        )?;

        // Migrations for databases created before these columns existed
        conn.execute_batch(
            "ALTER TABLE records ADD COLUMN is_trashed INTEGER NOT NULL DEFAULT 0;"
        ).ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN media_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN thumb_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN width INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN height INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN content_html TEXT;").ok();
        conn.execute_batch(
            "ALTER TABLE records ADD COLUMN content_len INTEGER NOT NULL DEFAULT 0;",
        )
        .ok();
        conn.execute_batch(
            "ALTER TABLE records ADD COLUMN alias TEXT NOT NULL DEFAULT '';",
        )
        .ok();
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_records_trashed_updated
             ON records(is_trashed, updated_at DESC);",
        )
        .ok();
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_records_trashed_pinned_updated
             ON records(is_trashed, is_pinned, updated_at DESC);",
        )
        .ok();
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_records_hash_active
             ON records(hash, is_trashed);",
        )
        .ok();

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

            -- tag_id lookups (delete_tag, auto-tag refresh) scan record_tags.
            CREATE INDEX IF NOT EXISTS idx_record_tags_tag_id ON record_tags(tag_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT OR IGNORE INTO tags (name, color, is_auto) VALUES
                ('部署', '#22c55e', 1),
                ('前端', '#6366f1', 1),
                ('链接', '#eab308', 1),
                ('重要', '#ef4444', 0),
                ('设计', '#a855f7', 0);
            "#,
        )?;

        // One-time backfill of content_len (avoids length(content) on every list query).
        let backfilled: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'content_len_backfill'",
                [],
                |row| row.get(0),
            )
            .ok();
        if backfilled.as_deref() != Some("1") {
            let _ = conn.execute(
                "UPDATE records SET content_len = length(content) WHERE content_len = 0",
                [],
            );
            let _ = conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('content_len_backfill', '1')",
                [],
            );
        }

        // Snap legacy tag hex values onto the fixed 12-color hue wheel.
        tags::migrate_tag_palette_v2(&conn)?;

        // --- Schema version gate ---
        // All migrations above are idempotent (CREATE IF NOT EXISTS / ALTER … .ok()).
        // After they run, stamp the expected version so doctor can verify it.
        Self::apply_schema_version(&conn)?;

        Self::ensure_fts(&conn)?;

        media::ensure_dirs(&media_root).ok();

        // Reader pool: open after schema is ready (same DB file, WAL).
        let mut read_conns = Vec::with_capacity(READ_POOL_SIZE);
        for _ in 0..READ_POOL_SIZE {
            let c = Connection::open(db_path)?;
            Self::configure_connection(&c, true)?;
            read_conns.push(Mutex::new(c));
        }

        // M-3: Pre-compute string prefix for enrich_paths (avoids per-record PathBuf).
        let media_root_prefix = {
            let mut s = media_root.to_string_lossy().to_string();
            if !s.ends_with('\\') && !s.ends_with('/') {
                s.push('\\');
            }
            s
        };

        Ok(Self {
            conn: Mutex::new(conn),
            read_conns,
            read_rr: std::sync::atomic::AtomicUsize::new(0),
            media_root,
            media_root_prefix,
            settings_cache: RwLock::new(None),
        })
    }

    #[inline]
    pub(super) fn lock_read(&self) -> parking_lot::MutexGuard<'_, Connection> {
        let n = self.read_conns.len();
        let start = self
            .read_rr
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % n;
        for o in 0..n {
            if let Some(g) = self.read_conns[(start + o) % n].try_lock() {
                return g;
            }
        }
        // L-5: All readers busy — log contention for diagnostics before blocking.
        tracing::debug!("DB read pool exhausted ({} conns); blocking on lock", n);
        self.read_conns[start].lock()
    }

    pub fn media_root(&self) -> &Path {
        &self.media_root
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(ClipboardDb::escape_like("100%"), "100\\%");
        assert_eq!(ClipboardDb::escape_like("a_b"), "a\\_b");
        assert_eq!(ClipboardDb::escape_like("c:\\x"), "c:\\\\x");
        assert_eq!(ClipboardDb::escape_like("plain"), "plain");
    }

    #[test]
    fn fts_match_needs_three_chars() {
        assert_eq!(ClipboardDb::build_fts_match("ab"), None);
        assert_eq!(ClipboardDb::build_fts_match("  x "), None);
        assert_eq!(ClipboardDb::build_fts_match("abc"), Some("\"abc\"".to_string()));
    }

    #[test]
    fn fts_match_escapes_quotes() {
        assert_eq!(
            ClipboardDb::build_fts_match(r#"a"b"#),
            Some("\"a\"\"b\"".to_string())
        );
    }

    #[test]
    fn placeholders_join_count() {
        assert_eq!(ClipboardDb::id_placeholders(0), "");
        assert_eq!(ClipboardDb::id_placeholders(1), "?");
        assert_eq!(ClipboardDb::id_placeholders(3), "?,?,?");
    }

    // ── Schema compatibility tests ──────────────────────────────────

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
        // Migrations (idempotent ALTER TABLE … .ok())
        conn.execute_batch("ALTER TABLE records ADD COLUMN is_trashed INTEGER NOT NULL DEFAULT 0;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN media_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN thumb_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN width INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN height INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN content_html TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN content_len INTEGER NOT NULL DEFAULT 0;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN alias TEXT NOT NULL DEFAULT '';").ok();
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_records_trashed_updated ON records(is_trashed, updated_at DESC);").ok();
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_records_trashed_pinned_updated ON records(is_trashed, is_pinned, updated_at DESC);").ok();
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_records_hash_active ON records(hash, is_trashed);").ok();
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

        // Apply the same idempotent migrations from ClipboardDb::new
        conn.execute_batch("ALTER TABLE records ADD COLUMN is_trashed INTEGER NOT NULL DEFAULT 0;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN media_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN thumb_path TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN width INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN height INTEGER;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN content_html TEXT;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN content_len INTEGER NOT NULL DEFAULT 0;").ok();
        conn.execute_batch("ALTER TABLE records ADD COLUMN alias TEXT NOT NULL DEFAULT '';").ok();

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
        use super::{RECORD_COLS, RECORD_COLS_LIST};
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
