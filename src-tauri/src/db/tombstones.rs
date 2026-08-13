//! WebDAV deletion tombstones — cross-device delete propagation.
//!
//! When a record is explicitly deleted (trash / permanent delete / batch /
//! empty trash / clear history), a tombstone `(hash, deleted_at)` is recorded
//! locally and published by the next WebDAV push. Other devices apply it by
//! moving their (older) copy to the trash, so deletions stop resurrecting.
//!
//! Rules that keep this safe:
//! - Newer-wins: a local active record whose `updated_at` is *newer* than the
//!   tombstone's `deleted_at` (a deliberate re-copy) is kept, and the stale
//!   tombstone is dropped.
//! - Recipients trash (never hard-delete), so deletions stay recoverable.
//! - Automatic cleanup (max-records eviction, sensitive expiry) does NOT write
//!   tombstones — those are local capacity/privacy policies.
//! - Tombstones are garbage-collected remotely only after every known device
//!   has acknowledged applying them (min-ack watermark), preventing accidental
//!   resurrection from an un-synced device.

use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

use super::ClipboardDb;

/// Settings-table key holding the newest remote tombstone this device applied.
/// `clear_all_data` drops it alongside the tombstone rows so a cleared device
/// treats the next pull as a fresh join instead of re-propagating deletions.
pub(super) const ACK_KEY: &str = "webdav_tombstone_ack";

impl ClipboardDb {
    /// Insert or refresh a deletion tombstone. Keeps the *latest* `deleted_at`
    /// and the conservative (ever-sensitive) flag across conflicting writes.
    pub(super) fn upsert_tombstone_conn(
        conn: &Connection,
        hash: &str,
        deleted_at: &str,
        is_sensitive: bool,
    ) -> SqlResult<()> {
        conn.execute(
            "INSERT INTO sync_tombstones (hash, deleted_at, is_sensitive) VALUES (?1, ?2, ?3)
             ON CONFLICT(hash) DO UPDATE SET
                deleted_at = MAX(sync_tombstones.deleted_at, excluded.deleted_at),
                is_sensitive = MAX(sync_tombstones.is_sensitive, excluded.is_sensitive)",
            params![hash, deleted_at, is_sensitive as i32],
        )?;
        Ok(())
    }

    /// Record a tombstone under the write lock (delete-path hook).
    pub fn upsert_tombstone(
        &self,
        hash: &str,
        deleted_at: &str,
        is_sensitive: bool,
    ) -> SqlResult<()> {
        let conn = self.conn.lock();
        Self::upsert_tombstone_conn(&conn, hash, deleted_at, is_sensitive)
    }

    /// Drop a tombstone — called on restore (un-delete propagation) and when a
    /// push proves a newer active copy supersedes it.
    pub fn remove_tombstone(&self, hash: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM sync_tombstones WHERE hash = ?", [hash])?;
        Ok(())
    }

    /// All local tombstones: `(hash, deleted_at, is_sensitive)`.
    pub fn get_sync_tombstones(&self) -> SqlResult<Vec<(String, String, bool)>> {
        let conn = self.lock_read();
        let mut stmt =
            conn.prepare("SELECT hash, deleted_at, is_sensitive FROM sync_tombstones")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0))
        })?;
        rows.collect()
    }

    /// Watermark of the newest remote tombstone this device has applied.
    pub fn get_tombstone_ack(&self) -> SqlResult<Option<String>> {
        let conn = self.lock_read();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?",
            [ACK_KEY],
            |row| row.get(0),
        )
        .optional()
    }

    fn set_tombstone_ack_conn(conn: &Connection, ts: &str) -> SqlResult<()> {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            params![ACK_KEY, ts],
        )?;
        Ok(())
    }

    /// Apply remote tombstones after a pull. For each tombstone:
    /// - an active local copy strictly older than `deleted_at` is moved to the
    ///   trash (fresh local trash clock; the tombstone timestamp is preserved);
    /// - a newer local copy (re-copy) is kept and any stale local tombstone for
    ///   the hash is dropped;
    /// - the device ack watermark advances past every tombstone seen.
    ///
    /// Returns `(trashed_count, new_ack)`.
    pub fn apply_remote_tombstones(
        &self,
        tombstones: &[(String, String)],
    ) -> SqlResult<(usize, Option<String>)> {
        let conn = self.conn.lock();
        let mut applied = 0usize;
        let mut ack: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?",
                [ACK_KEY],
                |row| row.get(0),
            )
            .optional()?;

        for (hash, deleted_at) in tombstones {
            // Sensitivity is known only when the record still exists locally;
            // carry it so the sync_sensitive=false push filter still applies.
            let row: Option<(i64, String, i32)> = conn
                .query_row(
                    "SELECT id, updated_at, is_sensitive FROM records
                     WHERE hash = ?1 AND is_trashed = 0
                     ORDER BY updated_at DESC LIMIT 1",
                    [hash],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            match row {
                Some((id, updated_at, is_sensitive)) if updated_at < *deleted_at => {
                    Self::upsert_tombstone_conn(&conn, hash, deleted_at, is_sensitive != 0)?;
                    let now = chrono::Utc::now().to_rfc3339();
                    conn.execute(
                        "UPDATE records SET is_trashed = 1, is_pinned = 0,
                         auto_expire_at = NULL, updated_at = ? WHERE id = ?",
                        params![now, id],
                    )?;
                    applied += 1;
                }
                Some((_, updated_at, is_sensitive)) => {
                    // Newer local copy wins; drop any stale local tombstone.
                    Self::upsert_tombstone_conn(&conn, hash, deleted_at, is_sensitive != 0)?;
                    conn.execute(
                        "DELETE FROM sync_tombstones WHERE hash = ? AND deleted_at <= ?",
                        params![hash, updated_at],
                    )?;
                }
                None => {
                    // Record never existed locally (or only in trash) — just
                    // persist the tombstone so a later re-copy is handled.
                    Self::upsert_tombstone_conn(&conn, hash, deleted_at, false)?;
                }
            }
            ack = Some(match ack {
                Some(a) if a >= *deleted_at => a,
                _ => deleted_at.clone(),
            });
        }

        if let Some(ts) = &ack {
            Self::set_tombstone_ack_conn(&conn, ts)?;
        }
        Ok((applied, ack))
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;
    use crate::ClipboardRecord;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "clipvault_tombstone_test_{}_{}",
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

    fn make_record(content: &str, hash: &str, updated_at: &str) -> ClipboardRecord {
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
            is_trashed: false,
            auto_expire_at: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            tags: vec![],
            tag_colors: Vec::new(),
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

    /// Import re-derives text hashes (sha256(sha256(content))) — tombstones
    /// must reference the same identity the record actually has in the DB.
    fn derived_hash(content: &str) -> String {
        crate::detect::sha256_hash(&crate::detect::sha256_hash(content))
    }

    #[test]
    fn upsert_keeps_latest_deleted_at() {
        let (db, dir) = temp_db();
        db.upsert_tombstone("h1", "2026-01-02T00:00:00Z", false)
            .unwrap();
        db.upsert_tombstone("h1", "2026-01-01T00:00:00Z", true)
            .unwrap();
        let rows = db.get_sync_tombstones().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "2026-01-02T00:00:00Z");
        assert!(rows[0].2, "ever-sensitive flag must be conservative");
        cleanup(dir);
    }

    #[test]
    fn apply_tombstone_trashes_older_copy_and_acks() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record("old copy", "tomb-hash", "2026-01-01T00:00:00Z")],
            100,
            None,
        )
        .unwrap();

        let (trashed, ack) = db
            .apply_remote_tombstones(&[(
                derived_hash("old copy"),
                "2026-02-01T00:00:00Z".to_string(),
            )])
            .unwrap();

        assert_eq!(trashed, 1);
        assert_eq!(ack.as_deref(), Some("2026-02-01T00:00:00Z"));
        assert_eq!(db.get_trash_count().unwrap(), 1);
        // Tombstone persisted with the remote deletion time, not the local trash time.
        let rows = db.get_sync_tombstones().unwrap();
        assert_eq!(rows[0].1, "2026-02-01T00:00:00Z");
        // Ack watermark persisted for the next push.
        assert_eq!(
            db.get_tombstone_ack().unwrap().as_deref(),
            Some("2026-02-01T00:00:00Z")
        );
        cleanup(dir);
    }

    #[test]
    fn apply_tombstone_keeps_newer_recopy() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record(
                "fresh copy",
                "tomb-hash",
                "2026-03-01T00:00:00Z",
            )],
            100,
            None,
        )
        .unwrap();

        let (trashed, _) = db
            .apply_remote_tombstones(&[(
                derived_hash("fresh copy"),
                "2026-02-01T00:00:00Z".to_string(),
            )])
            .unwrap();

        assert_eq!(trashed, 0);
        assert_eq!(db.get_records_for_export(10, 0).unwrap().len(), 1);
        // The stale tombstone (older than the surviving copy) is dropped.
        assert!(db.get_sync_tombstones().unwrap().is_empty());
        cleanup(dir);
    }

    #[test]
    fn restore_removes_tombstone_and_bumps_updated_at() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record("data", "restore-hash", "2026-01-01T00:00:00Z")],
            100,
            None,
        )
        .unwrap();
        let id = db.get_records_for_export(10, 0).unwrap()[0].id;
        db.trash_record(id).unwrap();
        assert!(!db.get_sync_tombstones().unwrap().is_empty());

        std::thread::sleep(std::time::Duration::from_millis(5));
        db.restore_record(id).unwrap();

        assert!(db.get_sync_tombstones().unwrap().is_empty());
        let conn = db.conn.lock();
        let updated_at: String = conn
            .query_row("SELECT updated_at FROM records WHERE id = ?", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(updated_at.as_str() > "2026-01-01T00:00:00Z");
        cleanup(dir);
    }
}
