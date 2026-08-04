use rusqlite::{Connection, Result as SqlResult};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::media;
use crate::Settings;

mod records_import;
mod records_media;
mod records_query;
mod records_search;
mod records_write;
mod schema;
mod settings;
mod stats;
mod tags;
mod types;

// Schema compatibility tests live in `schema_tests.rs` (test-only module) to
// keep schema.rs under the 500-line cap.
#[cfg(test)]
mod schema_tests;

pub use types::{ContentType, ImageMeta, ALIAS_MAX_CHARS, RECORD_COLS, RECORD_COLS_LIST};

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
                alias TEXT NOT NULL DEFAULT '',
                source_name TEXT NOT NULL DEFAULT ''
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

        // Schema-aware migrations: only ALTER for columns that are genuinely
        // missing (checked via PRAGMA table_info). The previous
        // `ALTER TABLE ... .ok()` swallowed every failure silently — including
        // real ones — leaving the schema half-migrated. CREATE INDEX IF NOT
        // EXISTS is idempotent and errors propagate.
        Self::migrate_schema(&conn)?;

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
            conn.execute(
                "UPDATE records SET content_len = length(content) WHERE content_len = 0",
                [],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('content_len_backfill', '1')",
                [],
            )?;
        }

        // Snap legacy tag hex values onto the fixed 12-color hue wheel.
        tags::migrate_tag_palette_v2(&conn)?;

        // --- Schema version gate ---
        // All migrations above are idempotent (CREATE IF NOT EXISTS / ALTER … .ok()).
        // After they run, stamp the expected version so doctor can verify it.
        Self::apply_schema_version(&conn)?;

        Self::ensure_fts(&conn)?;

        // media/ dirs were already created by the caller (lib.rs::run) before
        // the DB opens; a failure here is recorded rather than swallowed so a
        // missing media root is visible in logs instead of failing later at
        // image-capture time with no trace.
        if let Err(e) = media::ensure_dirs(&media_root) {
            tracing::warn!("Failed to create media directories: {}", e);
        }

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

