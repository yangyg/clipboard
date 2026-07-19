use rusqlite::{Connection, Result as SqlResult, params, Row};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::fmt;
use crate::media;
use crate::{ClipboardRecord, Settings, StatsData, TagInfo};

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

/// Full row including rich HTML (detail / paste / emit).
const RECORD_COLS: &str = "id, content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, content_html";

/// List/search omit heavy content_html (NULL) — preview lazy-loads via get_record.
const RECORD_COLS_LIST: &str = "id, content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, NULL as content_html";

pub struct ClipboardDb {
    conn: Mutex<Connection>,
    media_root: PathBuf,
}

impl ClipboardDb {
    pub fn new(db_path: &Path, media_root: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                content_type TEXT NOT NULL DEFAULT 'text',
                source_app TEXT NOT NULL DEFAULT '',
                source_window TEXT NOT NULL DEFAULT '',
                hash TEXT NOT NULL,
                copy_count INTEGER NOT NULL DEFAULT 1,
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
                content_html TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_records_updated_at ON records(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_hash ON records(hash);
            CREATE INDEX IF NOT EXISTS idx_records_content_type ON records(content_type);
            CREATE INDEX IF NOT EXISTS idx_records_is_favorite ON records(is_favorite);
            CREATE INDEX IF NOT EXISTS idx_records_trashed_updated
                ON records(is_trashed, updated_at DESC);"#,
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
            "CREATE INDEX IF NOT EXISTS idx_records_trashed_updated
             ON records(is_trashed, updated_at DESC);",
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

        media::ensure_dirs(&media_root).ok();

        Ok(Self {
            conn: Mutex::new(conn),
            media_root,
        })
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
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
            placeholders.join(",")
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
        let placeholders: Vec<String> = record_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT rt.record_id, t.name FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id IN ({})
             ORDER BY rt.record_id",
            placeholders.join(",")
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
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.conn.lock();
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

        if trashed {
            sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");
        } else {
            sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC LIMIT ? OFFSET ?");
        }
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

    pub fn get_record(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.conn.lock();
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
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM records WHERE hash = ? ORDER BY updated_at DESC LIMIT 1",
                [hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE records SET updated_at = ?, copy_count = copy_count + 1, source_app = ?, source_window = ? WHERE id = ?",
                params![now, source_app, source_window, id],
            )?;
            return Ok(id);
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
            "INSERT INTO records (content, content_type, source_app, source_window, hash, is_sensitive, auto_expire_at, created_at, updated_at, media_path, thumb_path, width, height, content_html)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            ],
        )?;

        let id = conn.last_insert_rowid();

        // Collect media of records about to be evicted by max_records
        let overflow_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                 ORDER BY updated_at ASC
                 LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)",
            )?;
            let ids = stmt
                .query_map([max_records.max(1)], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let overflow_media = self.fetch_media_paths_by_ids(&conn, &overflow_ids)?;

        conn.execute(
            "DELETE FROM records WHERE id IN (
                SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                ORDER BY updated_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)
            )",
            [max_records.max(1)],
        )?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);

        Ok(id)
    }

    pub fn search_records(
        &self,
        query: &str,
        limit: i32,
        offset: i32,
        content_type: Option<&str>,
        favorites_only: bool,
        tag_name: Option<&str>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.conn.lock();
        let search = format!("%{}%", query);
        let mut sql = format!(
            "SELECT {} FROM records
             WHERE is_trashed = 0 AND (content LIKE ? OR source_app LIKE ?)",
            RECORD_COLS_LIST
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(search.clone()), Box::new(search)];

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
        sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC LIMIT ? OFFSET ?");
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
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM records WHERE id IN ({})",
            placeholders.join(",")
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
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "UPDATE records SET is_trashed = 1, is_pinned = 0 WHERE id IN ({})",
            placeholders.join(",")
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
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "UPDATE records SET is_trashed = 0 WHERE id IN ({})",
            placeholders.join(",")
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
        let conn = self.conn.lock();
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

    pub fn get_settings(&self) -> SqlResult<Settings> {
        let conn = self.conn.lock();
        let mut settings = Settings::default();

        if let Ok(json) = conn.query_row::<String, _, _>(
            "SELECT value FROM settings WHERE key = 'app_settings'",
            [],
            |row| row.get(0),
        ) {
            if let Ok(s) = serde_json::from_str::<Settings>(&json) {
                settings = s;
            }
        }

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &Settings) -> SqlResult<()> {
        let conn = self.conn.lock();
        let json = serde_json::to_string(settings).unwrap_or_default();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('app_settings', ?)",
            [&json],
        )?;
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

    pub fn cleanup_expired(&self) -> SqlResult<()> {
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
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute(
            "DELETE FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?",
            [now],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
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
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut imported = 0;

        for record in records {
            // Skip empty text records; image records may have empty content with media_path
            let is_image = record.content_type == "image";
            if (!is_image && record.content.trim().is_empty()) || record.hash.trim().is_empty() {
                continue;
            }

            let exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM records WHERE hash = ?",
                [&record.hash],
                |row| row.get(0),
            )?;

            if exists > 0 {
                continue;
            }

            tx.execute(
                "INSERT INTO records (
                    content, content_type, source_app, source_window, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                ],
            )?;
            imported += 1;
        }

        let overflow_ids: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                 ORDER BY updated_at ASC
                 LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)",
            )?;
            let ids = stmt
                .query_map([max_records.max(1)], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let overflow_media: Vec<(Option<String>, Option<String>)> = {
            if overflow_ids.is_empty() {
                Vec::new()
            } else {
                let placeholders: Vec<String> = overflow_ids.iter().map(|_| "?".to_string()).collect();
                let sql = format!(
                    "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
                    placeholders.join(",")
                );
                let params: Vec<&dyn rusqlite::types::ToSql> =
                    overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                let mut stmt = tx.prepare(&sql)?;
                let pairs = stmt
                    .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
                    .filter_map(|r| r.ok())
                    .collect();
                pairs
            }
        };

        tx.execute(
            "DELETE FROM records WHERE id IN (
                SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                ORDER BY updated_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)
            )",
            [max_records.max(1)],
        )?;
        tx.commit()?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);
        Ok(imported)
    }

    // === Tag CRUD ===

    pub fn get_all_tags(&self) -> SqlResult<Vec<TagInfo>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.color, t.is_auto, COUNT(rt.record_id) as cnt
             FROM tags t
             LEFT JOIN record_tags rt ON rt.tag_id = t.id
             GROUP BY t.id
             ORDER BY t.name",
        )?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    is_auto: row.get::<_, i32>(3)? != 0,
                    count: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?, ?)",
            params![name, color],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_tag(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tags WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn update_tag(&self, id: i64, name: &str, color: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tags SET name = ?, color = ? WHERE id = ?",
            params![name, color, id],
        )?;
        Ok(())
    }

    pub fn add_tag_to_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
            params![record_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_tag_from_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM record_tags WHERE record_id = ? AND tag_id = ?",
            params![record_id, tag_id],
        )?;
        Ok(())
    }

    // === Stats ===

    pub fn get_stats(&self) -> SqlResult<StatsData> {
        let conn = self.conn.lock();
        let total_records = conn.query_row("SELECT COUNT(*) FROM records WHERE is_trashed = 0", [], |row| row.get(0))?;
        let total_copies = conn.query_row("SELECT COALESCE(SUM(copy_count), 0) FROM records WHERE is_trashed = 0", [], |row| row.get(0))?;
        let favorites_count = conn.query_row("SELECT COUNT(*) FROM records WHERE is_favorite = 1 AND is_trashed = 0", [], |row| row.get(0))?;
        let pinned_count = conn.query_row("SELECT COUNT(*) FROM records WHERE is_pinned = 1 AND is_trashed = 0", [], |row| row.get(0))?;
        let sensitive_count = conn.query_row("SELECT COUNT(*) FROM records WHERE is_sensitive = 1 AND is_trashed = 0", [], |row| row.get(0))?;
        let content_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(length(content)), 0) + COALESCE(SUM(length(COALESCE(content_html, ''))), 0)
             FROM records WHERE is_trashed = 0",
            [],
            |row| row.get(0),
        )?;

        let mut type_distribution = std::collections::HashMap::new();
        let mut stmt = conn.prepare("SELECT content_type, COUNT(*) FROM records WHERE is_trashed = 0 GROUP BY content_type")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
        for row in rows {
            let (content_type, count) = row?;
            type_distribution.insert(content_type, count);
        }
        drop(stmt);
        drop(conn);

        let media_bytes = media_dir_size(&self.media_root);
        let storage_bytes = content_bytes.saturating_add(media_bytes);

        Ok(StatsData {
            total_records,
            total_copies,
            favorites_count,
            pinned_count,
            sensitive_count,
            storage_bytes,
            type_distribution,
        })
    }
}

fn media_dir_size(root: &std::path::Path) -> i64 {
    fn walk(dir: &std::path::Path, acc: &mut u64) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, acc);
            } else if let Ok(meta) = entry.metadata() {
                *acc = acc.saturating_add(meta.len());
            }
        }
    }
    let mut total = 0u64;
    walk(root, &mut total);
    total.min(i64::MAX as u64) as i64
}
