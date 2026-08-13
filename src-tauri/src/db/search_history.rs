//! Search-history persistence (autocomplete dropdown).
//! Local-only: this data is deliberately excluded from export/import and WebDAV
//! sync. One row per distinct query, recency-ordered by `last_searched_at`.
use rusqlite::Result as SqlResult;

use super::ClipboardDb;
use crate::SearchHistoryEntry;

/// Soft cap for stored rows. The dropdown only surfaces `HISTORY_DISPLAY_MAX`,
/// but keeping more rows locally is cheap and future-proofs management/stats.
const HISTORY_STORE_MAX: i64 = 50;

impl ClipboardDb {
    /// Recency-ordered search history (most recent first).
    pub fn get_search_history(&self, limit: i64) -> SqlResult<Vec<SearchHistoryEntry>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(
            "SELECT query, search_count, last_searched_at
             FROM search_history
             ORDER BY last_searched_at DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map([limit.clamp(0, i64::from(super::MAX_PAGE_SIZE))], |row| {
            Ok(SearchHistoryEntry {
                query: row.get(0)?,
                search_count: row.get(1)?,
                last_searched_at: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Upsert a query: existing rows bump `search_count` and move to the front;
    /// new rows are inserted. Trims to `HISTORY_STORE_MAX` rows.
    pub fn record_search_history(&self, query: &str) -> SqlResult<()> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO search_history (query, search_count, last_searched_at)
             VALUES (?1, 1, ?2)
             ON CONFLICT(query) DO UPDATE SET
                search_count = search_count + 1,
                last_searched_at = ?2",
            rusqlite::params![query, now],
        )?;
        // Keep storage bounded. ORDER BY last_searched_at DESC matches the
        // display order, so deleting non-matching rows trims the oldest.
        conn.execute(
            "DELETE FROM search_history WHERE query NOT IN (
                SELECT query FROM search_history
                ORDER BY last_searched_at DESC LIMIT ?
             )",
            [HISTORY_STORE_MAX],
        )?;
        Ok(())
    }

    pub fn remove_search_history(&self, query: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM search_history WHERE query = ?",
            rusqlite::params![query],
        )?;
        Ok(())
    }

    pub fn clear_search_history(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM search_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::ClipboardDb;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        crate::db::test_util::temp_db("search_history")
    }

    fn cleanup(dir: PathBuf) {
        crate::db::test_util::cleanup(dir)
    }

    #[test]
    fn record_moves_existing_to_front_and_bumps_count() {
        let (db, dir) = temp_db();
        db.record_search_history("alpha").unwrap();
        db.record_search_history("beta").unwrap();
        db.record_search_history("gamma").unwrap();

        let before = db.get_search_history(50).unwrap();
        assert_eq!(
            before.iter().map(|e| e.query.as_str()).collect::<Vec<_>>(),
            ["gamma", "beta", "alpha"]
        );

        // Re-search "alpha" → bump count, move to front.
        db.record_search_history("alpha").unwrap();
        let after = db.get_search_history(50).unwrap();
        assert_eq!(after[0].query, "alpha");
        assert_eq!(after[0].search_count, 2);
        assert_eq!(after.len(), 3);
        cleanup(dir);
    }

    #[test]
    fn blank_query_is_ignored() {
        let (db, dir) = temp_db();
        db.record_search_history("  ").unwrap();
        assert!(db.get_search_history(50).unwrap().is_empty());
        cleanup(dir);
    }

    #[test]
    fn store_is_trimmed_to_cap() {
        let (db, dir) = temp_db();
        for i in 0..60 {
            db.record_search_history(&format!("query-{i}")).unwrap();
        }
        let rows = db.get_search_history(200).unwrap();
        assert_eq!(rows.len(), 50);
        // Newest 50 kept; the oldest 10 (query-0..query-9) trimmed.
        assert_eq!(rows[0].query, "query-59");
        assert!(!rows.iter().any(|e| e.query == "query-9"));
        cleanup(dir);
    }

    #[test]
    fn remove_and_clear_work() {
        let (db, dir) = temp_db();
        db.record_search_history("alpha").unwrap();
        db.record_search_history("beta").unwrap();

        db.remove_search_history("alpha").unwrap();
        let rows = db.get_search_history(50).unwrap();
        assert_eq!(
            rows.iter().map(|e| e.query.as_str()).collect::<Vec<_>>(),
            ["beta"]
        );

        db.clear_search_history().unwrap();
        assert!(db.get_search_history(50).unwrap().is_empty());
        cleanup(dir);
    }
}
