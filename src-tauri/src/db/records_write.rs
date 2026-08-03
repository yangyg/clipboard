//! Record inserts, soft-delete/trash, restore, favorites, pin, alias.
use rusqlite::{params, Result as SqlResult};

use super::{ClipboardDb, ContentType, ImageMeta, ALIAS_MAX_CHARS};
use crate::ClipboardRecord;

impl ClipboardDb {
    // === Insert ===

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
        // workers; single writer Mutex serializes capture + UI mutations). A real
        // read error here must not be mistaken for "no match" — that would insert
        // a duplicate row instead of deduping.
        let existing: Option<i64> = match conn.query_row(
            "SELECT id FROM records WHERE hash = ? AND is_trashed = 0
             ORDER BY updated_at DESC LIMIT 1",
            [hash],
            |row| row.get(0),
        ) {
            Ok(id) => Some(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };

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
                    .collect::<SqlResult<Vec<_>>>()?;
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

    // === Delete / Trash ===

    pub fn delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        conn.execute("DELETE FROM records WHERE id = ?", [id])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

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

    pub fn permanently_delete_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, ids)?;
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "DELETE FROM records WHERE is_trashed = 1 AND id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        drop(conn);
        if count > 0 {
            self.purge_media_pairs(&media);
        }
        Ok(count)
    }

    pub fn empty_trash(&self) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM records WHERE is_trashed = 1")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
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

    // === Favorites / Pin / Alias ===

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
        let placeholders = Self::id_placeholders(ids.len());
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

    // === Bulk cleanup ===

    pub fn clear_non_favorite(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_trashed = 0",
            )?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute("DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0", [])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }
}
