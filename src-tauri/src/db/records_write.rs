//! Record inserts, soft-delete/trash, restore, favorites, pin, alias.
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

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
            // FTS indexes source_app/source_window, but the content-only
            // trigger (`AFTER UPDATE OF content`) never fires on a dedup
            // re-copy — refresh the FTS row only when the source actually
            // changed, so searching by the new source still matches.
            let source_changed: bool = conn.query_row(
                "SELECT source_app != ?1 OR source_window != ?2 OR source_name != ?3
                 FROM records WHERE id = ?4",
                params![source_app, source_window, source_name, id],
                |row| row.get(0),
            )?;
            // Re-copy only refreshes source/timestamp — paste count is separate.
            conn.execute(
                "UPDATE records SET updated_at = ?, source_app = ?, source_window = ?, source_name = ? WHERE id = ?",
                params![now, source_app, source_window, source_name, id],
            )?;
            if source_changed {
                Self::refresh_record_fts(&conn, id)?;
            }
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

    /// Cheap existence probe used by startup history import to skip records
    /// that already exist (active **or** trashed). Deliberately distinct from
    /// `insert_record`'s dedup-update path: importing an existing item must NOT
    /// bump `updated_at` or reset `source_*` to empty (re-ranking the list every
    /// session). Any-row matching mirrors the `UNIQUE(hash)` index — a hash that
    /// only exists in the trash would otherwise slip the probe and make the
    /// subsequent `insert_record` fail with a UNIQUE constraint violation.
    pub fn record_hash_exists(&self, hash: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM records WHERE hash = ? LIMIT 1",
            [hash],
            |_| Ok(()),
        )
        .map(|_| true)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })
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
        // Clear the sensitive auto-expiry too: the record now belongs to the
        // trash lifecycle, not the capture-expiry one.
        // Record a deletion tombstone so WebDAV can propagate the delete.
        let Some((hash, is_sensitive)) = conn
            .query_row(
                "SELECT hash, is_sensitive FROM records WHERE id = ?",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? != 0)),
            )
            .optional()?
        else {
            return Ok(());
        };
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE records SET is_trashed = 1, is_pinned = 0, auto_expire_at = NULL, updated_at = ? WHERE id = ?",
            params![now, id],
        )?;
        Self::upsert_tombstone_conn(&conn, &hash, &now, is_sensitive)?;
        Ok(())
    }

    pub fn trash_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 1, is_pinned = 0, auto_expire_at = NULL, updated_at = ? WHERE id IN ({})",
            placeholders
        );
        let now = chrono::Utc::now().to_rfc3339();
        let rows: Vec<(String, bool)> = {
            let mut stmt = conn.prepare(&format!(
                "SELECT hash, is_sensitive FROM records WHERE id IN ({placeholders})"
            ))?;
            let params: Vec<&dyn rusqlite::types::ToSql> = ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt
                .query_map(params.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? != 0))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(ids.len() + 1);
        params.push(&now);
        params.extend(ids.iter().map(|id| id as &dyn rusqlite::types::ToSql));
        let count = conn.execute(&sql, params.as_slice())?;
        for (hash, is_sensitive) in rows {
            Self::upsert_tombstone_conn(&conn, &hash, &now, is_sensitive)?;
        }
        Ok(count)
    }

    pub fn restore_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let hash: Option<String> = conn
            .query_row("SELECT hash FROM records WHERE id = ?", [id], |row| {
                row.get(0)
            })
            .optional()?;
        // Bump `updated_at` so a restore is "newer than the deletion" and beats
        // any remote tombstone on the next push (un-delete propagation).
        conn.execute(
            "UPDATE records SET is_trashed = 0, updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), id],
        )?;
        if let Some(hash) = hash {
            conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [hash])?;
        }
        Ok(())
    }

    pub fn restore_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 0, updated_at = ? WHERE id IN ({})",
            placeholders
        );
        let now = chrono::Utc::now().to_rfc3339();
        let hashes: Vec<String> = {
            let mut stmt = conn.prepare(&format!(
                "SELECT hash FROM records WHERE id IN ({placeholders})"
            ))?;
            let params: Vec<&dyn rusqlite::types::ToSql> = ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt
                .query_map(params.as_slice(), |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(ids.len() + 1);
        params.push(&now);
        params.extend(ids.iter().map(|id| id as &dyn rusqlite::types::ToSql));
        let count = conn.execute(&sql, params.as_slice())?;
        for hash in hashes {
            conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [hash])?;
        }
        Ok(count)
    }

    pub fn permanently_delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        // Only trashed rows can be permanently deleted; keep their tombstone
        // (already written on trash) and fill gaps for legacy pre-tombstone rows.
        let info: Option<(String, bool)> = conn
            .query_row(
                "SELECT hash, is_sensitive FROM records WHERE id = ? AND is_trashed = 1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? != 0)),
            )
            .optional()?;
        let n = conn.execute("DELETE FROM records WHERE id = ? AND is_trashed = 1", [id])?;
        drop(conn);
        if n > 0 {
            if let Some((hash, is_sensitive)) = info {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = self.upsert_tombstone(&hash, &now, is_sensitive);
            }
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
        let rows: Vec<(String, bool)> = {
            let placeholders = Self::id_placeholders(ids.len());
            let mut stmt = conn.prepare(&format!(
                "SELECT hash, is_sensitive FROM records
                 WHERE is_trashed = 1 AND id IN ({placeholders})"
            ))?;
            let params: Vec<&dyn rusqlite::types::ToSql> = ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt
                .query_map(params.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? != 0))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
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
            let now = chrono::Utc::now().to_rfc3339();
            for (hash, is_sensitive) in rows {
                let _ = self.upsert_tombstone(&hash, &now, is_sensitive);
            }
            self.purge_media_pairs(&media);
        }
        Ok(count)
    }

    pub fn empty_trash(&self) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let rows: Vec<(i64, String, bool)> = {
            let mut stmt =
                conn.prepare("SELECT id, hash, is_sensitive FROM records WHERE is_trashed = 1")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i32>(2)? != 0,
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        let now = chrono::Utc::now().to_rfc3339();
        for (_, hash, is_sensitive) in &rows {
            Self::upsert_tombstone_conn(&conn, hash, &now, *is_sensitive)?;
        }
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
        let rows: Vec<(i64, String, bool)> = {
            let mut stmt = conn.prepare(
                "SELECT id, hash, is_sensitive FROM records
                     WHERE is_favorite = 0 AND is_trashed = 0",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i32>(2)? != 0,
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            rows
        };
        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        let now = chrono::Utc::now().to_rfc3339();
        for (_, hash, is_sensitive) in &rows {
            Self::upsert_tombstone_conn(&conn, hash, &now, *is_sensitive)?;
        }
        conn.execute(
            "DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0",
            [],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;
    use crate::db::ContentType;
    use crate::detect::{sha256_hash, sha256_hash_bytes};
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "clipvault_records_write_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
        (db, dir)
    }

    fn cleanup(dir: PathBuf) {
        for name in ["test.db", "test.db-wal", "test.db-shm"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    fn insert(db: &ClipboardDb, content: &str) -> (i64, bool, crate::ClipboardRecord) {
        // Mirror the capture pipeline's double-hash text format.
        let hash = sha256_hash(&sha256_hash(content));
        db.insert_record(
            content,
            &ContentType::Text,
            &hash,
            false,
            1000,
            600,
            "app.exe",
            "win",
            "",
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn record_hash_exists_matches_active_and_trashed_rows() {
        let (db, dir) = temp_db();
        let (id, is_new, _) = insert(&db, "hello world");
        assert!(is_new);
        let text_hash = sha256_hash(&sha256_hash("hello world"));
        assert!(db.record_hash_exists(&text_hash).unwrap());
        assert!(!db
            .record_hash_exists(&sha256_hash(&sha256_hash("absent")))
            .unwrap());

        // Trashed rows still hold the UNIQUE(hash) slot, so a history re-import
        // must treat them as "already exists" — inserting again would trip the
        // UNIQUE constraint even though `is_trashed` is set.
        db.trash_record(id).unwrap();
        assert!(db.record_hash_exists(&text_hash).unwrap());
        cleanup(dir);
    }

    #[test]
    fn record_hash_exists_false_for_image_hash_and_empty_db() {
        let (db, dir) = temp_db();
        let image_hash = sha256_hash_bytes(&[1u8, 2, 3, 4]);
        assert!(!db.record_hash_exists(&image_hash).unwrap());
        cleanup(dir);
    }
}
