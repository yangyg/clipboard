//! App-settings persistence and scheduled cleanup (expired / retention).
use std::sync::Arc;

use rusqlite::Result as SqlResult;

use super::ClipboardDb;
use crate::Settings;

impl ClipboardDb {
    /// Returns a shared reference-counted Settings snapshot. Clone is cheap (Arc bump).
    /// Callers needing mutation (resize persist, webdav sync) should clone the inner Settings.
    pub fn get_settings(&self) -> SqlResult<Arc<Settings>> {
        if let Some(cached) = self.settings_cache.read().as_ref() {
            return Ok(Arc::clone(cached));
        }

        let mut settings = Settings::default();
        {
            let conn = self.lock_read();
            if let Ok(json) = conn.query_row::<String, _, _>(
                "SELECT value FROM settings WHERE key = 'app_settings'",
                [],
                |row| row.get(0),
            ) {
                if let Ok(s) = serde_json::from_str::<Settings>(&json) {
                    settings = s;
                }
            }
        }

        let arc = Arc::new(settings);
        *self.settings_cache.write() = Some(Arc::clone(&arc));
        Ok(arc)
    }

    pub fn save_settings(&self, settings: &Settings) -> SqlResult<()> {
        // A serialize failure must not write "" — the next load would silently
        // reset every setting to defaults. Fail loud instead.
        let json = serde_json::to_string(settings)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app_settings', ?)",
                [&json],
            )?;
        }
        *self.settings_cache.write() = Some(Arc::new(settings.clone()));
        Ok(())
    }

    pub fn cleanup_expired(&self) -> SqlResult<Vec<i64>> {
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
        if ids.is_empty() {
            return Ok(ids);
        }
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute(
            "DELETE FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?",
            [now],
        )?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(ids)
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
}
