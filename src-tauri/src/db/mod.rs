use rusqlite::{Connection, Result as SqlResult, params, Row};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::fmt;
use crate::media;
use crate::{ClipboardRecord, Settings};

mod tags;
mod stats;

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
    /// In-memory copy of `app_settings`, populated lazily. Avoids re-parsing the
    /// settings JSON on every clipboard event (the monitor reads it 2-3x/event).
    settings_cache: RwLock<Option<Settings>>,
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

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT OR IGNORE INTO tags (name, color, is_auto) VALUES
                ('部署', '#34d399', 1),
                ('前端', '#6366f1', 1),
                ('链接', '#fbbf24', 1),
                ('重要', '#f87171', 0),
                ('设计', '#a78bfa', 0);
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

        Self::ensure_fts(&conn)?;

        media::ensure_dirs(&media_root).ok();

        // Reader pool: open after schema is ready (same DB file, WAL).
        let mut read_conns = Vec::with_capacity(READ_POOL_SIZE);
        for _ in 0..READ_POOL_SIZE {
            let c = Connection::open(db_path)?;
            Self::configure_connection(&c, true)?;
            read_conns.push(Mutex::new(c));
        }

        Ok(Self {
            conn: Mutex::new(conn),
            read_conns,
            read_rr: std::sync::atomic::AtomicUsize::new(0),
            media_root,
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
        self.read_conns[start].lock()
    }

    /// Escape `%`, `_`, `\` for use with `LIKE … ESCAPE '\'`.
    /// Kept for unit tests; production short search uses `instr` (no LIKE wildcards).
    #[cfg(test)]
    fn escape_like(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    }

    /// FTS5 trigram: substring MATCH needs ≥3 chars. Returns quoted token.
    fn build_fts_match(query: &str) -> Option<String> {
        let q = query.trim();
        if q.chars().count() < 3 {
            return None;
        }
        Some(format!("\"{}\"", q.replace('"', "\"\"")))
    }

    /// Short (1–2 char) search: one `instr` pass over records + tag EXISTS.
    /// Avoids leading-wildcard `LIKE '%X%'` which cannot use indexes and multiplies scans.
    fn push_short_query_predicate(
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
        query: &str,
    ) {
        sql.push_str(
            "instr(content, ?) > 0
             OR instr(alias, ?) > 0
             OR instr(source_app, ?) > 0
             OR instr(source_window, ?) > 0
             OR EXISTS (
                SELECT 1 FROM record_tags rt
                INNER JOIN tags t ON t.id = rt.tag_id
                WHERE rt.record_id = records.id AND instr(t.name, ?) > 0
             )",
        );
        let q = query.to_string();
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        params.push(Box::new(q));
    }

    /// Whitelist sort keys → ORDER BY fragment. Unknown values fall back to updated_desc.
    /// Non-trash lists keep pinned rows first.
    fn order_by_clause(trashed: bool, sort: Option<&str>) -> &'static str {
        let secondary = match sort.unwrap_or("updated_desc") {
            "updated_asc" => "updated_at ASC",
            "created_desc" => "created_at DESC",
            "copies_desc" => "copy_count DESC, updated_at DESC",
            _ => "updated_at DESC",
        };
        if trashed {
            return secondary;
        }
        match secondary {
            "updated_at ASC" => "is_pinned DESC, updated_at ASC",
            "created_at DESC" => "is_pinned DESC, created_at DESC",
            "copy_count DESC, updated_at DESC" => "is_pinned DESC, copy_count DESC, updated_at DESC",
            _ => "is_pinned DESC, updated_at DESC",
        }
    }

    /// Build a comma-joined `?,?,…` placeholder list for an `IN (…)` clause.
    fn id_placeholders(n: usize) -> String {
        std::iter::repeat("?").take(n).collect::<Vec<_>>().join(",")
    }

    fn ensure_fts(conn: &Connection) -> SqlResult<()> {
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

    pub fn media_root(&self) -> &Path {
        &self.media_root
    }

    fn enrich_paths(&self, media_path: Option<String>, thumb_path: Option<String>) -> (Option<String>, Option<String>) {
        let to_abs = |rel: &str| {
            media::absolute(&self.media_root, rel)
                .to_string_lossy()
                .to_string()
        };
        let media_abs = media_path.as_deref().map(to_abs);
        let thumb_abs = thumb_path.as_deref().map(to_abs);
        (media_abs, thumb_abs)
    }

    fn map_record_row(&self, row: &Row<'_>) -> SqlResult<ClipboardRecord> {
        let media_path: Option<String> = row.get(14)?;
        let thumb_path: Option<String> = row.get(15)?;
        let (media_abs, thumb_abs) = self.enrich_paths(media_path.clone(), thumb_path.clone());
        Ok(ClipboardRecord {
            id: row.get(0)?,
            content: row.get(1)?,
            content_type: row.get(2)?,
            source_app: row.get(3)?,
            source_window: row.get(4)?,
            hash: row.get(5)?,
            copy_count: row.get(6)?,
            is_favorite: row.get(7)?,
            is_pinned: row.get(8)?,
            is_sensitive: row.get(9)?,
            is_trashed: row.get(10)?,
            auto_expire_at: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            tags: Vec::new(),
            media_path,
            thumb_path,
            width: row.get(16)?,
            height: row.get(17)?,
            content_html: row.get(18)?,
            media_abs,
            thumb_abs,
            content_len: row.get(19).ok(),
            alias: row.get::<_, String>(20).unwrap_or_default(),
        })
    }

    fn purge_media_pairs(&self, pairs: &[(Option<String>, Option<String>)]) {
        for (media_path, thumb_path) in pairs {
            media::delete_media_files(
                &self.media_root,
                media_path.as_deref(),
                thumb_path.as_deref(),
            );
        }
    }

    fn fetch_media_paths_by_ids(
        &self,
        conn: &Connection,
        ids: &[i64],
    ) -> SqlResult<Vec<(Option<String>, Option<String>)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql)?;
        let pairs = stmt
            .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(pairs)
    }

    /// Batch-load tags for multiple record IDs in one query.
    fn load_tags_batch(&self, conn: &Connection, record_ids: &[i64]) -> SqlResult<std::collections::HashMap<i64, Vec<String>>> {
        if record_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = Self::id_placeholders(record_ids.len());
        let sql = format!(
            "SELECT rt.record_id, t.name FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id IN ({})
             ORDER BY rt.record_id",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            record_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut map: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
        for id in record_ids {
            map.entry(*id).or_default();
        }
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (rid, tag_name) = row?;
            if let Some(tags) = map.get_mut(&rid) {
                tags.push(tag_name);
            }
        }
        Ok(map)
    }

    pub fn get_records(
        &self,
        limit: i32,
        offset: i32,
        trashed: bool,
        content_type: Option<&str>,
        favorites_only: bool,
        tag_name: Option<&str>,
        sort: Option<&str>,
        // Keyset cursor (preferred over OFFSET when list mutates via prepend).
        before_pinned: Option<i32>,
        before_updated_at: Option<&str>,
        before_id: Option<i64>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut sql = format!(
            "SELECT {} FROM records WHERE is_trashed = ?",
            RECORD_COLS_LIST
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(if trashed { 1i32 } else { 0i32 })];

        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND content_type = ?");
            params.push(Box::new(ct.to_string()));
        }
        if favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }
        if let Some(tag) = tag_name.filter(|s| !s.is_empty()) {
            sql.push_str(
                " AND id IN (
                    SELECT rt.record_id FROM record_tags rt
                    INNER JOIN tags t ON t.id = rt.tag_id
                    WHERE t.name = ?
                )",
            );
            params.push(Box::new(tag.to_string()));
        }

        // Keyset for default newest-first (+ pinned). Avoids OFFSET drift when
        // clipboard-changed prepends rows while the user scrolls.
        let use_keyset = before_id.is_some()
            && before_updated_at.is_some()
            && matches!(sort.unwrap_or("updated_desc"), "updated_desc");

        if use_keyset {
            let pin = before_pinned.unwrap_or(0);
            let ts = before_updated_at.unwrap().to_string();
            let id = before_id.unwrap();
            // ORDER BY is_pinned DESC, updated_at DESC, id DESC → next page
            sql.push_str(
                " AND (
                    is_pinned < ?
                    OR (is_pinned = ? AND updated_at < ?)
                    OR (is_pinned = ? AND updated_at = ? AND id < ?)
                )",
            );
            params.push(Box::new(pin));
            params.push(Box::new(pin));
            params.push(Box::new(ts.clone()));
            params.push(Box::new(pin));
            params.push(Box::new(ts));
            params.push(Box::new(id));
            sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC, id DESC LIMIT ?");
            params.push(Box::new(limit.max(1)));
        } else {
            sql.push_str(" ORDER BY ");
            sql.push_str(Self::order_by_clause(trashed, sort));
            sql.push_str(" LIMIT ? OFFSET ?");
            params.push(Box::new(limit.max(1)));
            params.push(Box::new(offset.max(0)));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(param_refs.as_slice(), |row| self.map_record_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let tags_map = self.load_tags_batch(&conn, &ids)?;
        for record in &mut records {
            if let Some(tags) = tags_map.get(&record.id) {
                record.tags = tags.clone();
            }
        }

        Ok(records)
    }

    pub fn get_record(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE id = ?",
            RECORD_COLS
        ))?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut record = self.map_record_row(row)?;
            record.tags = self.get_record_tags_locked(&conn, record.id)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// List-shaped row (truncated content, no HTML) — cheaper emit after capture.
    pub fn get_record_list(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.lock_read();
        self.get_record_list_locked(&conn, id)
    }

    fn get_record_list_locked(
        &self,
        conn: &Connection,
        id: i64,
    ) -> SqlResult<Option<ClipboardRecord>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE id = ?",
            RECORD_COLS_LIST
        ))?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut record = self.map_record_row(row)?;
            record.tags = self.get_record_tags_locked(conn, record.id)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Tag names for a record (read lock). Used after auto-tag without reloading the row.
    pub fn get_record_tag_names(&self, record_id: i64) -> SqlResult<Vec<String>> {
        let conn = self.lock_read();
        self.get_record_tags_locked(&conn, record_id)
    }

    /// Full record + bump copy_count in one write lock (paste hot path).
    pub fn take_record_for_paste(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.conn.lock();
        let mut record = {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM records WHERE id = ? AND is_trashed = 0",
                RECORD_COLS
            ))?;
            let mut rows = stmt.query([id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            self.map_record_row(row)?
        };
        record.tags = self.get_record_tags_locked(&conn, record.id)?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE records SET copy_count = copy_count + 1, updated_at = ? WHERE id = ?",
            params![now, id],
        )?;
        record.copy_count = record.copy_count.saturating_add(1);
        Ok(Some(record))
    }

    fn get_record_tags_locked(&self, conn: &Connection, record_id: i64) -> SqlResult<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT t.name FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id = ?",
        )?;
        let tags = stmt
            .query_map([record_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    }

    pub fn insert_record(
        &self,
        content: &str,
        content_type: &ContentType,
        hash: &str,
        is_sensitive: bool,
        max_records: i32,
        sensitive_auto_expire_seconds: i32,
        source_app: &str,
        source_window: &str,
        image: Option<&ImageMeta>,
        content_html: Option<&str>,
    ) -> SqlResult<(i64, bool, ClipboardRecord)> {
        let conn = self.conn.lock();

        // Hash check + insert/update under the same write lock (no TOCTOU between
        // workers; single writer Mutex serializes capture + UI mutations).
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM records WHERE hash = ? AND is_trashed = 0
                 ORDER BY updated_at DESC LIMIT 1",
                [hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            let now = chrono::Utc::now().to_rfc3339();
            // Re-copy only refreshes source/timestamp — paste count is separate.
            conn.execute(
                "UPDATE records SET updated_at = ?, source_app = ?, source_window = ? WHERE id = ?",
                params![now, source_app, source_window, id],
            )?;
            let record = self
                .get_record_list_locked(&conn, id)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
            return Ok((id, false, record));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let auto_expire_at = if is_sensitive && sensitive_auto_expire_seconds > 0 {
            Some((chrono::Utc::now() + chrono::Duration::seconds(sensitive_auto_expire_seconds as i64)).to_rfc3339())
        } else {
            None
        };

        let (media_path, thumb_path, width, height) = match image {
            Some(img) => (
                Some(img.media_path.as_str()),
                Some(img.thumb_path.as_str()),
                Some(img.width),
                Some(img.height),
            ),
            None => (None, None, None, None),
        };

        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, hash, copy_count, is_sensitive, auto_expire_at, created_at, updated_at, media_path, thumb_path, width, height, content_html, content_len)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                content,
                content_type.as_str(),
                source_app,
                source_window,
                hash,
                is_sensitive as i32,
                auto_expire_at,
                now,
                now,
                media_path,
                thumb_path,
                width,
                height,
                content_html,
                content.chars().count() as i64,
            ],
        )?;

        let id = conn.last_insert_rowid();

        // Cheap over-cap probe (scan ≤ max+1 rows). Only then pay for a full COUNT.
        let max = max_records.max(1) as i64;
        let over_cap: bool = conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM records WHERE is_trashed = 0 LIMIT ?
             )",
            [max + 1],
            |row| row.get::<_, i64>(0),
        )? > max;
        if over_cap {
            let active_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM records WHERE is_trashed = 0",
                [],
                |row| row.get(0),
            )?;
            let overflow_count = (active_count - max).max(0);
            // Collect media of records about to be evicted by max_records
            let overflow_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                     ORDER BY updated_at ASC LIMIT ?",
                )?;
                let ids = stmt
                    .query_map([overflow_count], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                ids
            };
            let overflow_media = self.fetch_media_paths_by_ids(&conn, &overflow_ids)?;

            if !overflow_ids.is_empty() {
                let placeholders = Self::id_placeholders(overflow_ids.len());
                let params: Vec<&dyn rusqlite::types::ToSql> =
                    overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                conn.execute(
                    &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                    params.as_slice(),
                )?;
            }
            let record = self
                .get_record_list_locked(&conn, id)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
            drop(conn);
            self.purge_media_pairs(&overflow_media);
            return Ok((id, true, record));
        }

        let record = self
            .get_record_list_locked(&conn, id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        Ok((id, true, record))
    }

    pub fn search_records(
        &self,
        query: &str,
        limit: i32,
        offset: i32,
        content_type: Option<&str>,
        favorites_only: bool,
        tag_name: Option<&str>,
        sort: Option<&str>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.lock_read();
        let mut sql = format!(
            "SELECT {} FROM records WHERE is_trashed = 0 AND (",
            RECORD_COLS_LIST
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // ≥3 chars: FTS5 trigram. Shorter: single-pass instr (no LIKE '%…%').
        if let Some(fts_match) = Self::build_fts_match(query) {
            sql.push_str("id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?)");
            params.push(Box::new(fts_match));
        } else {
            Self::push_short_query_predicate(&mut sql, &mut params, query);
        }
        sql.push(')');

        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND content_type = ?");
            params.push(Box::new(ct.to_string()));
        }
        if favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }
        if let Some(tag) = tag_name.filter(|s| !s.is_empty()) {
            sql.push_str(
                " AND id IN (
                    SELECT rt.record_id FROM record_tags rt
                    INNER JOIN tags t ON t.id = rt.tag_id
                    WHERE t.name = ?
                )",
            );
            params.push(Box::new(tag.to_string()));
        }
        sql.push_str(" ORDER BY ");
        sql.push_str(Self::order_by_clause(false, sort));
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit.max(1)));
        params.push(Box::new(offset.max(0)));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(param_refs.as_slice(), |row| self.map_record_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let tags_map = self.load_tags_batch(&conn, &ids)?;
        for record in &mut records {
            if let Some(tags) = tags_map.get(&record.id) {
                record.tags = tags.clone();
            }
        }

        Ok(records)
    }

    pub fn delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        conn.execute("DELETE FROM records WHERE id = ?", [id])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

    pub fn delete_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, ids)?;
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "DELETE FROM records WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(count)
    }

    // === Trash / Soft-delete (keep media until permanent delete) ===

    pub fn trash_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE records SET is_trashed = 1, is_pinned = 0 WHERE id = ?",
            [id],
        )?;
        Ok(())
    }

    pub fn trash_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 1, is_pinned = 0 WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        Ok(count)
    }

    pub fn restore_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("UPDATE records SET is_trashed = 0 WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn restore_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 0 WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        Ok(count)
    }

    pub fn permanently_delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        let n = conn.execute("DELETE FROM records WHERE id = ? AND is_trashed = 1", [id])?;
        drop(conn);
        if n > 0 {
            self.purge_media_pairs(&media);
        }
        Ok(())
    }

    pub fn empty_trash(&self) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM records WHERE is_trashed = 1")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        let count = conn.execute("DELETE FROM records WHERE is_trashed = 1", [])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(count)
    }

    pub fn get_trash_count(&self) -> SqlResult<i64> {
        let conn = self.lock_read();
        conn.query_row("SELECT COUNT(*) FROM records WHERE is_trashed = 1", [], |row| row.get(0))
    }

    pub fn increment_copy_count(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE records SET copy_count = copy_count + 1, updated_at = ? WHERE id = ?",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn toggle_favorite(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let current: i32 = conn.query_row(
            "SELECT is_favorite FROM records WHERE id = ?",
            [id],
            |row| row.get(0),
        )?;
        let new_val = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE records SET is_favorite = ? WHERE id = ?",
            params![new_val, id],
        )?;
        Ok(new_val == 1)
    }

    pub fn batch_set_favorite(&self, ids: &[i64], favorite: bool) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(if favorite { 1i32 } else { 0i32 })];
        for id in ids {
            params.push(Box::new(*id));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let n = conn.execute(
            &format!(
                "UPDATE records SET is_favorite = ? WHERE id IN ({placeholders})"
            ),
            param_refs.as_slice(),
        )?;
        Ok(n)
    }

    pub fn toggle_pin(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let current: i32 =
            conn.query_row("SELECT is_pinned FROM records WHERE id = ?", [id], |row| {
                row.get(0)
            })?;
        let new_val = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE records SET is_pinned = ? WHERE id = ?",
            params![new_val, id],
        )?;
        Ok(new_val == 1)
    }

    /// Set short display alias (trim + max 80 chars). Empty clears. Does not touch content/hash.
    pub fn set_record_alias(&self, id: i64, alias: &str) -> SqlResult<String> {
        let mut alias = alias.trim().to_string();
        if alias.chars().count() > ALIAS_MAX_CHARS {
            alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
        }
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE records SET alias = ? WHERE id = ?",
            params![alias, id],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Self::refresh_record_fts(&conn, id)?;
        Ok(alias)
    }

    pub fn get_settings(&self) -> SqlResult<Settings> {
        if let Some(cached) = self.settings_cache.read().as_ref() {
            return Ok(cached.clone());
        }

        let mut settings = Settings::default();
        {
            let conn = self.lock_read();
            if let Ok(json) = conn.query_row::<String, _, _>(
                "SELECT value FROM settings WHERE key = 'app_settings'",
                [],
                |row| row.get(0),
            ) {
                if let Ok(s) = serde_json::from_str::<Settings>(&json) {
                    settings = s;
                }
            }
        }

        *self.settings_cache.write() = Some(settings.clone());
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &Settings) -> SqlResult<()> {
        let json = serde_json::to_string(settings).unwrap_or_default();
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app_settings', ?)",
                [&json],
            )?;
        }
        *self.settings_cache.write() = Some(settings.clone());
        Ok(())
    }

    pub fn clear_non_favorite(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_trashed = 0",
            )?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute("DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0", [])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

    pub fn cleanup_expired(&self) -> SqlResult<Vec<i64>> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?",
            )?;
            let ids = stmt
                .query_map([&now], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        if ids.is_empty() {
            return Ok(ids);
        }
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute(
            "DELETE FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?",
            [now],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(ids)
    }

    pub fn cleanup_retention(&self, retention_days: i32) -> SqlResult<()> {
        if retention_days <= 0 {
            return Ok(());
        }
        let conn = self.conn.lock();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).to_rfc3339();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 1 AND updated_at < ?",
            )?;
            let ids = stmt
                .query_map([&cutoff], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute(
            "DELETE FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 1 AND updated_at < ?",
            [cutoff],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

    pub fn import_records(&self, records: &[ClipboardRecord], max_records: i32) -> SqlResult<i32> {
        let (imported, _) = self.import_records_with_merge(records, max_records)?;
        Ok(imported)
    }

    /// Import with hash dedup. Existing hashes get a shallow merge:
    /// newer `updated_at`, OR on favorite/pin, max `copy_count`, fill missing media paths.
    /// Returns `(inserted, merged)`.
    pub fn import_records_with_merge(
        &self,
        records: &[ClipboardRecord],
        max_records: i32,
    ) -> SqlResult<(i32, i32)> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut imported = 0;
        let mut merged = 0;

        // Batch-load existing hashes in one query instead of per-record lookups.
        let existing_hashes: std::collections::HashSet<String> = {
            let mut stmt = tx.prepare("SELECT hash FROM records")?;
            let hashes: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            hashes.into_iter().collect()
        };

        for record in records {
            let mut record = record.clone();
            record.content_type = crate::security::normalize_content_type(&record.content_type);
            if record.content_type == "link" && !crate::security::is_safe_http_url(&record.content) {
                record.content_type = "text".into();
            }
            if let Some(ref mp) = record.media_path {
                if !crate::security::is_allowed_media_rel(mp) {
                    record.media_path = None;
                    record.thumb_path = None;
                    record.media_abs = None;
                    record.thumb_abs = None;
                }
            }
            if let Some(ref tp) = record.thumb_path {
                if !crate::security::is_allowed_media_rel(tp) {
                    record.thumb_path = None;
                    record.thumb_abs = None;
                }
            }
            // Cap HTML blob size from malicious imports
            if let Some(ref html) = record.content_html {
                if html.len() > 512 * 1024 {
                    record.content_html = None;
                }
            }

            // Skip empty text records; image records may have empty content with media_path
            let is_image = record.content_type == "image";
            if (!is_image && record.content.trim().is_empty()) || record.hash.trim().is_empty() {
                continue;
            }

            if existing_hashes.contains(&record.hash) {
                let changed = tx.execute(
                    "UPDATE records SET
                        is_favorite = CASE WHEN is_favorite = 1 OR ? = 1 THEN 1 ELSE 0 END,
                        is_pinned = CASE WHEN is_pinned = 1 OR ? = 1 THEN 1 ELSE 0 END,
                        copy_count = CASE WHEN copy_count < ? THEN ? ELSE copy_count END,
                        updated_at = CASE WHEN updated_at < ? THEN ? ELSE updated_at END,
                        media_path = CASE
                            WHEN (media_path IS NULL OR media_path = '') AND ? IS NOT NULL AND ? != ''
                            THEN ? ELSE media_path END,
                        thumb_path = CASE
                            WHEN (thumb_path IS NULL OR thumb_path = '') AND ? IS NOT NULL AND ? != ''
                            THEN ? ELSE thumb_path END
                     WHERE hash = ?",
                    params![
                        record.is_favorite as i32,
                        record.is_pinned as i32,
                        record.copy_count,
                        record.copy_count,
                        record.updated_at,
                        record.updated_at,
                        record.media_path,
                        record.media_path,
                        record.media_path,
                        record.thumb_path,
                        record.thumb_path,
                        record.thumb_path,
                        record.hash,
                    ],
                )?;
                if changed > 0 {
                    merged += 1;
                }
                continue;
            }

            let mut alias = record.alias.trim().to_string();
            if alias.chars().count() > ALIAS_MAX_CHARS {
                alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
            }

            tx.execute(
                "INSERT INTO records (
                    content, content_type, source_app, source_window, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html, alias
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.content,
                    record.content_type,
                    record.source_app,
                    record.source_window,
                    record.hash,
                    record.copy_count,
                    record.is_favorite as i32,
                    record.is_pinned as i32,
                    record.is_sensitive as i32,
                    record.is_trashed as i32,
                    record.auto_expire_at,
                    record.created_at,
                    record.updated_at,
                    record.media_path,
                    record.thumb_path,
                    record.width,
                    record.height,
                    record.content_html,
                    alias,
                ],
            )?;
            imported += 1;
        }

        let active_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 0", [], |row| row.get(0),
        )?;
        let max = max_records.max(1) as i64;
        if active_count > max {
            let overflow_count = active_count - max;
            let overflow_ids: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                     ORDER BY updated_at ASC LIMIT ?",
                )?;
                let ids: Vec<i64> = stmt
                    .query_map([overflow_count], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                ids
            };
            let overflow_media: Vec<(Option<String>, Option<String>)> = {
                if overflow_ids.is_empty() {
                    Vec::new()
                } else {
                    let placeholders = Self::id_placeholders(overflow_ids.len());
                    let sql = format!(
                        "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
                        placeholders
                    );
                    let params: Vec<&dyn rusqlite::types::ToSql> =
                        overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                    let mut stmt = tx.prepare(&sql)?;
                    let pairs: Vec<(Option<String>, Option<String>)> = stmt
                        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
                        .filter_map(|r| r.ok())
                        .collect();
                    pairs
                }
            };

            if !overflow_ids.is_empty() {
                let placeholders = Self::id_placeholders(overflow_ids.len());
                let params: Vec<&dyn rusqlite::types::ToSql> =
                    overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                tx.execute(
                    &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                    params.as_slice(),
                )?;
            }
            tx.commit()?;
            drop(conn);
            self.purge_media_pairs(&overflow_media);
        } else {
            tx.commit()?;
        }
        Ok((imported, merged))
    }

    /// Full-content page for export/backup (never use list truncation columns).
    pub fn get_records_for_export(
        &self,
        limit: i32,
        offset: i32,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE is_trashed = 0
             ORDER BY is_pinned DESC, updated_at DESC LIMIT ? OFFSET ?",
            RECORD_COLS
        ))?;
        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(params![limit, offset], |row| self.map_record_row(row))?
            .collect::<SqlResult<Vec<_>>>()?;
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let tags_map = self.load_tags_batch(&conn, &ids)?;
        for record in &mut records {
            if let Some(tags) = tags_map.get(&record.id) {
                record.tags = tags.clone();
            }
        }
        Ok(records)
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
}
