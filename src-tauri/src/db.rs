use rusqlite::{Connection, Result as SqlResult, params};
use parking_lot::Mutex;
use std::path::Path;
use std::fmt;
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

pub struct ClipboardDb {
    conn: Mutex<Connection>,
}

impl ClipboardDb {
    pub fn new(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;

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
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_records_updated_at ON records(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_records_hash ON records(hash);
            CREATE INDEX IF NOT EXISTS idx_records_content_type ON records(content_type);
            CREATE INDEX IF NOT EXISTS idx_records_is_favorite ON records(is_favorite);"#,
        )?;

        // Migration: add is_trashed column for databases created before v0.2
        conn.execute_batch(
            "ALTER TABLE records ADD COLUMN is_trashed INTEGER NOT NULL DEFAULT 0;"
        ).ok();

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

        Ok(Self { conn: Mutex::new(conn) })
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

    pub fn get_records(&self, limit: i32, trashed: bool) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.conn.lock();
        let columns = "id, content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at";
        let sql = if trashed {
            format!("SELECT {} FROM records WHERE is_trashed = 1 ORDER BY updated_at DESC LIMIT ?", columns)
        } else {
            format!("SELECT {} FROM records WHERE is_trashed = 0 ORDER BY is_pinned DESC, updated_at DESC LIMIT ?", columns)
        };

        let mut stmt = conn.prepare(&sql)?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map([limit], |row| {
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
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Batch load tags
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
        let mut stmt = conn.prepare(
            "SELECT id, content, content_type, source_app, source_window, hash,
                    copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
                    created_at, updated_at
             FROM records WHERE id = ?",
        )?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut record = ClipboardRecord {
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
            };
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
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();

        // Check for duplicate by hash
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

        // Insert new record
        let now = chrono::Utc::now().to_rfc3339();
        let auto_expire_at = if is_sensitive && sensitive_auto_expire_seconds > 0 {
            Some((chrono::Utc::now() + chrono::Duration::seconds(sensitive_auto_expire_seconds as i64)).to_rfc3339())
        } else {
            None
        };
        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, hash, is_sensitive, auto_expire_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![content, content_type.as_str(), source_app, source_window, hash, is_sensitive as i32, auto_expire_at, now, now],
        )?;

        let id = conn.last_insert_rowid();

        // Enforce max records limit
        conn.execute(
            "DELETE FROM records WHERE id IN (
                SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                ORDER BY updated_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)
            )",
            [max_records.max(1)],
        )?;

        Ok(id)
    }

    pub fn search_records(&self, query: &str) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.conn.lock();
        let search = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, content, content_type, source_app, source_window, hash,
                    copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
                    created_at, updated_at
             FROM records
             WHERE is_trashed = 0 AND (content LIKE ? OR source_app LIKE ?)
             ORDER BY is_pinned DESC, updated_at DESC
             LIMIT 200",
        )?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map([&search, &search], |row| {
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
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Batch load tags
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
        conn.execute("DELETE FROM records WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn delete_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM records WHERE id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        Ok(count)
    }

    // === Trash / Soft-delete ===

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
        conn.execute("DELETE FROM records WHERE id = ? AND is_trashed = 1", [id])?;
        Ok(())
    }

    pub fn empty_trash(&self) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let count = conn.execute("DELETE FROM records WHERE is_trashed = 1", [])?;
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
        conn.execute("DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0", [])?;
        Ok(())
    }

    pub fn cleanup_expired(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "DELETE FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?",
            [now],
        )?;
        Ok(())
    }

    pub fn cleanup_retention(&self, retention_days: i32) -> SqlResult<()> {
        if retention_days <= 0 {
            return Ok(());
        }
        let conn = self.conn.lock();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).to_rfc3339();
        conn.execute(
            "DELETE FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 1 AND updated_at < ?",
            [cutoff],
        )?;
        Ok(())
    }

    pub fn import_records(&self, records: &[ClipboardRecord], max_records: i32) -> SqlResult<i32> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut imported = 0;

        for record in records {
            if record.content.trim().is_empty() || record.hash.trim().is_empty() {
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
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                ],
            )?;
            imported += 1;
        }

        tx.execute(
            "DELETE FROM records WHERE id IN (
                SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                ORDER BY updated_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM records WHERE is_trashed = 0) - ?)
            )",
            [max_records.max(1)],
        )?;
        tx.commit()?;
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
        let storage_bytes = conn.query_row("SELECT COALESCE(SUM(length(content)), 0) FROM records WHERE is_trashed = 0", [], |row| row.get(0))?;

        let mut type_distribution = std::collections::HashMap::new();
        let mut stmt = conn.prepare("SELECT content_type, COUNT(*) FROM records WHERE is_trashed = 0 GROUP BY content_type")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
        for row in rows {
            let (content_type, count) = row?;
            type_distribution.insert(content_type, count);
        }

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
