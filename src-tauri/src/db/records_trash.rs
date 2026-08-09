//! Soft-delete lifecycle: trash, restore, permanent delete, bulk cleanup.
//! All deletes write sync tombstones so WebDAV can propagate the deletion.
use rusqlite::{params, OptionalExtension, Result as SqlResult};

use super::ClipboardDb;

impl ClipboardDb {
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
        let rows = self.fetch_trash_metadata(&conn, ids)?;
        let now = chrono::Utc::now().to_rfc3339();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 1, is_pinned = 0, auto_expire_at = NULL, updated_at = ? WHERE id IN ({})",
            placeholders
        );
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(ids.len() + 1);
        params.push(&now);
        params.extend(ids.iter().map(|id| id as &dyn rusqlite::types::ToSql));
        let count = conn.execute(&sql, params.as_slice())?;
        for (hash, is_sensitive) in rows {
            Self::upsert_tombstone_conn(&conn, &hash, &now, is_sensitive)?;
        }
        Ok(count)
    }

    /// (hash, is_sensitive) for the given ids — tombstone inputs.
    fn fetch_trash_metadata(
        &self,
        conn: &rusqlite::Connection,
        ids: &[i64],
    ) -> SqlResult<Vec<(String, bool)>> {
        let placeholders = Self::id_placeholders(ids.len());
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
        Ok(rows)
    }

    pub fn restore_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let row: Option<(String, i32)> = conn
            .query_row(
                "SELECT hash, is_trashed FROM records WHERE id = ?",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
            )
            .optional()?;
        let Some((hash, trashed)) = row else {
            return Ok(());
        };
        // A re-copy may have claimed the hash slot while this row sat in the
        // trash. Restoring would then trip `uq_records_hash_active`; the fresh
        // active copy wins and the stale trashed row is dropped instead.
        if trashed != 0 && Self::hash_has_active_row(&conn, &hash)? {
            conn.execute("DELETE FROM records WHERE id = ?", [id])?;
            conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [&hash])?;
            return Ok(());
        }
        // Bump `updated_at` so a restore is "newer than the deletion" and beats
        // any remote tombstone on the next push (un-delete propagation).
        conn.execute(
            "UPDATE records SET is_trashed = 0, updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), id],
        )?;
        conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [&hash])?;
        Ok(())
    }

    fn hash_has_active_row(conn: &rusqlite::Connection, hash: &str) -> SqlResult<bool> {
        conn.query_row(
            "SELECT 1 FROM records WHERE hash = ? AND is_trashed = 0 LIMIT 1",
            [hash],
            |_| Ok(()),
        )
        .map(|_| true)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })
    }

    pub fn restore_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let hashes = self.fetch_hashes(&conn, ids)?;
        // Drop trashed rows whose hash was reclaimed by a fresh active copy
        // BEFORE the restore UPDATE: otherwise the partial unique index
        // `uq_records_hash_active` would abort the whole statement. Such rows
        // are stale duplicates that could never be restored anyway.
        Self::drop_stale_trashed_duplicates(&conn)?;
        let now = chrono::Utc::now().to_rfc3339();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 0, updated_at = ? WHERE id IN ({})",
            placeholders
        );
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(ids.len() + 1);
        params.push(&now);
        params.extend(ids.iter().map(|id| id as &dyn rusqlite::types::ToSql));
        let count = conn.execute(&sql, params.as_slice())?;
        for hash in hashes {
            conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [hash])?;
        }
        Ok(count)
    }

    fn fetch_hashes(&self, conn: &rusqlite::Connection, ids: &[i64]) -> SqlResult<Vec<String>> {
        let placeholders = Self::id_placeholders(ids.len());
        let mut stmt = conn.prepare(&format!(
            "SELECT hash FROM records WHERE id IN ({placeholders})"
        ))?;
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let hashes = stmt
            .query_map(params.as_slice(), |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(hashes)
    }

    /// Trashed rows whose hash an active row already holds can never be
    /// restored; sweeping them keeps the partial unique index satisfiable.
    fn drop_stale_trashed_duplicates(conn: &rusqlite::Connection) -> SqlResult<()> {
        conn.execute(
            "DELETE FROM records WHERE is_trashed = 1 AND hash IN (
                SELECT h.hash FROM records h WHERE h.is_trashed = 0
             )",
            [],
        )?;
        Ok(())
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
                // The row is already gone; a tombstone failure must not fail the
                // delete, but it MUST be visible — without it other devices
                // never learn about the deletion and resurrect the record.
                if let Err(e) = self.upsert_tombstone(&hash, &now, is_sensitive) {
                    tracing::warn!("Failed to write tombstone for {hash}: {e}");
                }
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
        let rows = self.fetch_trashed_metadata(&conn, ids)?;
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
                // See permanently_delete_record: log, never swallow silently.
                if let Err(e) = self.upsert_tombstone(&hash, &now, is_sensitive) {
                    tracing::warn!("Failed to write tombstone for {hash}: {e}");
                }
            }
            self.purge_media_pairs(&media);
        }
        Ok(count)
    }

    /// (hash, is_sensitive) for trashed rows among `ids` — tombstone inputs.
    fn fetch_trashed_metadata(
        &self,
        conn: &rusqlite::Connection,
        ids: &[i64],
    ) -> SqlResult<Vec<(String, bool)>> {
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
        Ok(rows)
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
