//! Record inserts, soft-delete/trash, restore, favorites, pin, alias.
use rusqlite::{params, Connection, Result as SqlResult};

use super::{ClipboardDb, ContentType, ImageMeta, ALIAS_MAX_CHARS};
use crate::detect::sha256_hash;
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
        source_name: &str,
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
                "UPDATE records SET updated_at = ?, source_app = ?, source_window = ?, source_name = ? WHERE id = ?",
                params![now, source_app, source_window, source_name, id],
            )?;
            let record = self
                .get_record_list_locked(&conn, id)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
            return Ok((id, false, record));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let auto_expire_at = if is_sensitive && sensitive_auto_expire_seconds > 0 {
            Some(
                (chrono::Utc::now()
                    + chrono::Duration::seconds(sensitive_auto_expire_seconds as i64))
                .to_rfc3339(),
            )
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
            "INSERT INTO records (content, content_type, source_app, source_window, source_name, hash, copy_count, is_sensitive, auto_expire_at, created_at, updated_at, media_path, thumb_path, width, height, content_html, content_len)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                content,
                content_type.as_str(),
                source_app,
                source_window,
                source_name,
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
            let overflow_media = self.evict_over_limit(&conn, max_records)?;
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

    /// Evict oldest non-favorite / non-pinned active rows when `max_records` is
    /// exceeded. Returns the media pairs of evicted rows; callers must release
    /// the write lock before passing them to `purge_media_pairs` (which takes a
    /// read lock). Shared by insert + import so capacity rules stay in sync.
    pub(super) fn evict_over_limit(
        &self,
        conn: &Connection,
        max_records: i32,
    ) -> SqlResult<Vec<(Option<String>, Option<String>)>> {
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 0",
            [],
            |row| row.get(0),
        )?;
        let max = max_records.max(1) as i64;
        if active_count <= max {
            return Ok(Vec::new());
        }
        let overflow_count = active_count - max;
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
        let overflow_media = self.fetch_media_paths_by_ids(conn, &overflow_ids)?;
        if !overflow_ids.is_empty() {
            let placeholders = Self::id_placeholders(overflow_ids.len());
            let params: Vec<&dyn rusqlite::types::ToSql> = overflow_ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(
                &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                params.as_slice(),
            )?;
        }
        Ok(overflow_media)
    }

    /// One-shot (settings flag `text_hash_v2`): re-derive text-record hashes
    /// from plain content and merge the duplicates the old scheme created.
    ///
    /// Historical hashes baked CF_HTML bytes into the fingerprint, so the same
    /// text copied from a different source (or re-written by our own paste)
    /// hashed differently and inserted a duplicate row. New identity is
    /// sha256(sha256(text)) — matching what capture stores now. Rows that
    /// collide after re-derivation are merged into the most recently updated
    /// one: favorite/pin OR'd, copy_count summed, tags unioned.
    pub(super) fn migrate_text_hash_v2(conn: &Connection) -> SqlResult<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'text_hash_v2'",
                [],
                |row| row.get(0),
            )
            .ok();
        if done.as_deref() == Some("1") {
            return Ok(());
        }

        // 1) Group candidate rows by the re-derived hash BEFORE writing
        // anything. Image rows hash pixels, not content — skip them. Updating
        // hashes row-by-row would trip UNIQUE(hash) mid-way: two legacy rows
        // with identical content re-derive to the same hash, so the second
        // UPDATE collides before any merge could run.
        // (id, is_favorite, is_pinned, copy_count, alias, is_trashed, updated_at)
        type LegacyRow = (i64, i32, i32, i32, String, i32, String);
        let mut groups: std::collections::HashMap<String, Vec<LegacyRow>> =
            std::collections::HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT id, content, is_favorite, is_pinned, copy_count, alias, is_trashed, updated_at
                 FROM records
                 WHERE content_type != 'image' AND media_path IS NULL",
            )?;
            let mapped = stmt.query_map([], |row| {
                let content: String = row.get(1)?;
                Ok((
                    sha256_hash(&sha256_hash(&content)),
                    (
                        row.get(0)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ),
                ))
            })?;
            for item in mapped {
                let (hash, entry) = item?;
                groups.entry(hash).or_default().push(entry);
            }
        }

        // 2) Per group: merge active duplicates into one survivor, delete the
        // rest, and apply the new hash last — with losers already gone the
        // UNIQUE(hash) constraint can never fire.
        for (new_hash, mut group) in groups {
            if group.len() == 1 {
                conn.execute(
                    "UPDATE records SET hash = ? WHERE id = ?",
                    params![new_hash, group[0].0],
                )?;
                continue;
            }
            // Winner: prefer active rows, then most recently updated.
            group.sort_by(|a, b| {
                a.5.cmp(&b.5) // is_trashed ASC (active first)
                    .then(b.6.cmp(&a.6)) // updated_at DESC
                    .then(b.0.cmp(&a.0)) // id DESC
            });
            let (winner_id, fav, pin, count, mut alias, _, _) = group.remove(0);
            let mut fav = fav != 0;
            let mut pin = pin != 0;
            let mut count = count;
            let mut loser_ids: Vec<i64> = Vec::new();
            for (id, f, p, c, a, trashed, _) in &group {
                loser_ids.push(*id);
                if *trashed == 0 {
                    // Only active rows contribute state; trashed dupes just vanish.
                    fav |= *f != 0;
                    pin |= *p != 0;
                    count += c;
                    if alias.is_empty() && !a.is_empty() {
                        alias = a.clone();
                    }
                }
            }
            conn.execute(
                "UPDATE records SET is_favorite = ?, is_pinned = ?, copy_count = ?, alias = ?
                 WHERE id = ?",
                params![fav as i32, pin as i32, count, alias, winner_id],
            )?;
            for loser in &loser_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO record_tags (record_id, tag_id)
                     SELECT ?, tag_id FROM record_tags WHERE record_id = ?",
                    params![winner_id, loser],
                )?;
            }
            Self::refresh_record_fts(conn, winner_id)?;
            // FTS row + record_tags links of losers cascade on delete.
            let placeholders = Self::id_placeholders(loser_ids.len());
            let params: Vec<&dyn rusqlite::types::ToSql> = loser_ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(
                &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                params.as_slice(),
            )?;
            conn.execute(
                "UPDATE records SET hash = ? WHERE id = ?",
                params![new_hash, winner_id],
            )?;
        }

        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('text_hash_v2', '1')",
            [],
        )?;
        Ok(())
    }

    // === Delete / Trash ===

    pub fn trash_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        // Bump `updated_at` so the trash-retention window (measured from
        // `updated_at`) starts at the moment of trashing — otherwise a record
        // copied weeks ago and trashed today is purged immediately.
        conn.execute(
            "UPDATE records SET is_trashed = 1, is_pinned = 0, updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), id],
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
            "UPDATE records SET is_trashed = 1, is_pinned = 0, updated_at = ? WHERE id IN ({})",
            placeholders
        );
        let now = chrono::Utc::now().to_rfc3339();
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(ids.len() + 1);
        params.push(&now);
        params.extend(ids.iter().map(|id| id as &dyn rusqlite::types::ToSql));
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
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
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
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
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
        conn.query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 1",
            [],
            |row| row.get(0),
        )
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
            &format!("UPDATE records SET is_favorite = ? WHERE id IN ({placeholders})"),
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
        // UPDATE + FTS refresh in one transaction: a crash between the two
        // would otherwise drop the FTS row permanently (search misses).
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE records SET alias = ? WHERE id = ?",
            params![alias, id],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Self::refresh_record_fts(&tx, id)?;
        tx.commit()?;
        Ok(alias)
    }

    // === Bulk cleanup ===

    pub fn clear_non_favorite(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt =
                conn.prepare("SELECT id FROM records WHERE is_favorite = 0 AND is_trashed = 0")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute(
            "DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0",
            [],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }
}
