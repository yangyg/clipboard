//! Soft-delete lifecycle: trash, restore, permanent delete, bulk cleanup.
//! All deletes write sync tombstones so WebDAV can propagate the deletion.
use rusqlite::{params, OptionalExtension, Result as SqlResult};

use super::schema::DEFAULT_TAGS_INSERT;
use super::{tombstones::ACK_KEY, ClipboardDb};

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

    /// Wipe every clipboard artifact: all records (active + trash, favorites,
    /// pinned, sensitive), media files, tags, search history, sync history and
    /// the WebDAV tombstone state. App settings survive. No tombstones are
    /// written — a cleared device joins the next WebDAV pull as fresh, so the
    /// wipe must not propagate spurious deletions to peers.
    pub fn clear_all_data(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        // Collect media references before the rows are gone so purge_media_pairs
        // can delete the files (it quarantines + re-checks, race-safe vs capture).
        let media: Vec<(Option<String>, Option<String>)> = {
            let mut stmt = conn.prepare(
                "SELECT media_path, thumb_path FROM records
                 WHERE media_path IS NOT NULL OR thumb_path IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<SqlResult<Vec<_>>>()?
        };
        // record_tags cascades via FK; records_fts rows go via the AFTER DELETE
        // trigger. Both require foreign_keys=ON (configure_connection sets it).
        conn.execute("DELETE FROM records", [])?;
        conn.execute("DELETE FROM tags", [])?;
        // Re-seed the built-in defaults so a fresh slate still ships them.
        conn.execute_batch(DEFAULT_TAGS_INSERT)?;
        conn.execute("DELETE FROM search_history", [])?;
        conn.execute("DELETE FROM sync_history", [])?;
        conn.execute("DELETE FROM sync_tombstones", [])?;
        conn.execute("DELETE FROM settings WHERE key = ?", [ACK_KEY])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;
    use crate::ClipboardRecord;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "clipvault_clear_all_test_{}_{}",
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

    fn make_record(content: &str, hash: &str, trashed: bool) -> ClipboardRecord {
        ClipboardRecord {
            id: 0,
            content: content.to_string(),
            content_type: "text".into(),
            source_app: String::new(),
            source_window: String::new(),
            source_name: String::new(),
            source_device_id: String::new(),
            hash: hash.to_string(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: trashed,
            auto_expire_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            tags: vec![],
            content_html: None,
            media_path: None,
            thumb_path: None,
            width: None,
            height: None,
            media_abs: None,
            thumb_abs: None,
            content_len: None,
            alias: String::new(),
        }
    }

    #[test]
    fn clear_all_data_wipes_records_media_tags_and_histories() {
        let (db, dir) = temp_db();
        // Media filenames must match the import guard (media/<64-hex>.png).
        let media_hash = "a".repeat(64);
        let media_rel = format!("media/{media_hash}.png");
        let thumb_rel = format!("media/thumbs/{media_hash}.jpg");

        // Media files on disk referenced by one record.
        std::fs::create_dir_all(dir.join("media/thumbs")).unwrap();
        std::fs::write(dir.join(&media_rel), b"png").unwrap();
        std::fs::write(dir.join(&thumb_rel), b"jpg").unwrap();

        let mut img = make_record("image copy", "img-hash", false);
        img.media_path = Some(media_rel.clone());
        img.thumb_path = Some(thumb_rel.clone());
        let mut trashed = make_record("deleted", "trash-hash", true);
        trashed.is_pinned = true;
        db.import_records_with_merge(
            &[make_record("hello", "txt-hash", false), img, trashed],
            100,
            None,
        )
        .unwrap();

        // A tag + assignment, search history, sync history, tombstone + ack.
        let tag_id = db.create_tag("user-tag", "#ff0000").unwrap();
        let records = db.get_records_for_export(10, 0).unwrap();
        let record_id = records[0].id;
        db.set_record_tags(record_id, &[tag_id]).unwrap();
        db.record_search_history("clipboard").unwrap();
        db.insert_sync_history("sync", true, 1, 0, 0, 0, 0, 0, 0, 0, None)
            .unwrap();
        db.upsert_tombstone("txt-hash", "2026-02-01T00:00:00Z", false)
            .unwrap();
        db.apply_remote_tombstones(&[("remote-hash".into(), "2026-02-02T00:00:00Z".into())])
            .unwrap(); // sets the ack watermark

        // App settings must survive — store one.
        db.conn
            .lock()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app_keep_me', '1')",
                [],
            )
            .unwrap();

        db.clear_all_data().unwrap();

        // Records (active + trash + favorites) all gone.
        assert!(db.get_records_for_export(10, 0).unwrap().is_empty());
        assert_eq!(db.get_trash_count().unwrap(), 0);
        // Media files deleted from disk.
        assert!(!dir.join(&media_rel).exists());
        assert!(!dir.join(&thumb_rel).exists());
        // Tags: only the re-seeded defaults remain (no user tag, no assignments).
        let tags = db.get_all_tags(None, false).unwrap();
        assert!(tags.len() == 5);
        assert!(!tags.iter().any(|t| t.name == "user-tag"));
        assert!(tags.iter().all(|t| t.count == 0));
        // Histories emptied.
        assert!(db.get_search_history(10).unwrap().is_empty());
        assert!(db.get_sync_history(10).unwrap().is_empty());
        // WebDAV tombstone state reset.
        assert!(db.get_sync_tombstones().unwrap().is_empty());
        assert!(db.get_tombstone_ack().unwrap().is_none());
        // App settings survive.
        let kept: Option<String> = db
            .conn
            .lock()
            .query_row(
                "SELECT value FROM settings WHERE key = 'app_keep_me'",
                [],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(kept.as_deref(), Some("1"));

        // Import re-derives text hashes; the seeded records use fake hashes, so
        // assert emptiness directly instead of via hash lookups.
        let count: i64 = db
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        cleanup(dir);
    }

    #[test]
    fn clear_all_data_is_idempotent_on_empty_db() {
        let (db, dir) = temp_db();
        db.clear_all_data().unwrap();
        db.clear_all_data().unwrap();
        assert!(db.get_records_for_export(10, 0).unwrap().is_empty());
        cleanup(dir);
    }

    #[test]
    fn clear_all_data_does_not_write_tombstones() {
        let (db, dir) = temp_db();
        let mut sensitive = make_record("sensitive copy", "sens-hash", false);
        sensitive.is_sensitive = true;
        db.import_records_with_merge(&[sensitive], 100, None)
            .unwrap();
        db.clear_all_data().unwrap();
        // A fresh-slate wipe must not publish deletions for cleared records.
        assert!(db.get_sync_tombstones().unwrap().is_empty());
        cleanup(dir);
    }
}
