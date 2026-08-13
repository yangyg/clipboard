//! WebDAV sync operation log. Local-only: this data is deliberately excluded
//! from export/import and WebDAV sync itself (recording would otherwise recurse).
//! One row per pull / push / sync run, newest-first with a soft cap.
use rusqlite::{params, Result as SqlResult};

use super::ClipboardDb;
use crate::SyncHistoryEntry;

/// Soft cap for stored rows. The UI only surfaces the newest handful, but
/// keeping a little more locally is cheap and future-proofs stats/debugging.
const SYNC_HISTORY_CAP: i64 = 50;

impl ClipboardDb {
    /// Append a sync operation and trim to the `SYNC_HISTORY_CAP` newest rows.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_sync_history(
        &self,
        action: &str,
        success: bool,
        pulled: i32,
        pushed: i32,
        merged: i32,
        tags_pulled: i32,
        tags_pushed: i32,
        media_downloaded: i32,
        media_uploaded: i32,
        media_skipped: i32,
        error: Option<&str>,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO sync_history (
                synced_at, action, success, pulled, pushed, merged,
                tags_pulled, tags_pushed, media_downloaded, media_uploaded, media_skipped, error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                chrono::Utc::now().to_rfc3339(),
                action,
                success as i32,
                pulled,
                pushed,
                merged,
                tags_pulled,
                tags_pushed,
                media_downloaded,
                media_uploaded,
                media_skipped,
                error,
            ],
        )?;
        let id = conn.last_insert_rowid();
        // Trim oldest rows (synced_at DESC + id tiebreak matches display order).
        conn.execute(
            "DELETE FROM sync_history WHERE id NOT IN (
                SELECT id FROM sync_history ORDER BY synced_at DESC, id DESC LIMIT ?
             )",
            [SYNC_HISTORY_CAP],
        )?;
        Ok(id)
    }

    /// Newest-first sync log (recency-ordered).
    pub fn get_sync_history(&self, limit: i64) -> SqlResult<Vec<SyncHistoryEntry>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(
            "SELECT id, synced_at, action, success, pulled, pushed, merged,
                    tags_pulled, tags_pushed, media_downloaded, media_uploaded, media_skipped, error
             FROM sync_history
             ORDER BY synced_at DESC, id DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map([limit.clamp(0, i64::from(super::MAX_PAGE_SIZE))], |row| {
            Ok(SyncHistoryEntry {
                id: row.get(0)?,
                synced_at: row.get(1)?,
                action: row.get(2)?,
                success: row.get::<_, i32>(3)? != 0,
                pulled: row.get(4)?,
                pushed: row.get(5)?,
                merged: row.get(6)?,
                tags_pulled: row.get(7)?,
                tags_pushed: row.get(8)?,
                media_downloaded: row.get(9)?,
                media_uploaded: row.get(10)?,
                media_skipped: row.get(11)?,
                error: row.get(12)?,
            })
        })?;
        rows.collect()
    }

    pub fn clear_sync_history(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM sync_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::ClipboardDb;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        crate::db::test_util::temp_db("sync_history")
    }

    fn cleanup(dir: PathBuf) {
        crate::db::test_util::cleanup(dir)
    }

    #[test]
    fn insert_and_query_round_trip() {
        let (db, dir) = temp_db();
        db.insert_sync_history("pull", true, 3, 0, 1, 2, 0, 1, 0, 0, None)
            .unwrap();
        db.insert_sync_history(
            "push",
            false,
            0,
            5,
            0,
            0,
            4,
            0,
            2,
            1,
            Some("401 Unauthorized"),
        )
        .unwrap();
        let rows = db.get_sync_history(10).unwrap();
        assert_eq!(rows.len(), 2);
        // Newest first (push inserted after pull).
        assert_eq!(rows[0].action, "push");
        assert!(!rows[0].success);
        assert_eq!(rows[0].pushed, 5);
        assert_eq!(rows[0].tags_pushed, 4);
        assert_eq!(rows[0].error.as_deref(), Some("401 Unauthorized"));
        assert_eq!(rows[1].action, "pull");
        assert!(rows[1].success);
        assert_eq!(rows[1].tags_pulled, 2);
        cleanup(dir);
    }

    #[test]
    fn store_is_trimmed_to_cap() {
        let (db, dir) = temp_db();
        for i in 0..60 {
            db.insert_sync_history("sync", true, i, 0, 0, 0, 0, 0, 0, 0, None)
                .unwrap();
        }
        let rows = db.get_sync_history(200).unwrap();
        assert_eq!(rows.len(), 50);
        // Newest 50 kept (highest `pulled` values).
        assert_eq!(rows[0].pulled, 59);
        assert!(!rows.iter().any(|e| e.pulled == 9));
        cleanup(dir);
    }

    #[test]
    fn clear_empties_log() {
        let (db, dir) = temp_db();
        db.insert_sync_history("sync", true, 1, 0, 0, 0, 0, 0, 0, 0, None)
            .unwrap();
        db.clear_sync_history().unwrap();
        assert!(db.get_sync_history(10).unwrap().is_empty());
        cleanup(dir);
    }
}
