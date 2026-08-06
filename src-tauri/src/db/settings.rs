//! App-settings persistence and scheduled cleanup (expired / retention).
use std::sync::Arc;

use rusqlite::Result as SqlResult;

use super::ClipboardDb;
use crate::Settings;

/// Errors that can occur while persisting settings. Kept separate from
/// `rusqlite::Result` so the DPAPI encryption step surfaces its own failure
/// type instead of being mislabeled as a SQL error.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings serialize failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("settings save failed: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("settings encryption failed: {0}")]
    Encryption(String),
}

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
            match conn.query_row::<String, _, _>(
                "SELECT value FROM settings WHERE key = 'app_settings'",
                [],
                |row| row.get(0),
            ) {
                Ok(json) => match serde_json::from_str::<Settings>(&json) {
                    Ok(mut s) => {
                        // WebDAV password is DPAPI-encrypted at rest; legacy
                        // plaintext values (no prefix) pass through unchanged.
                        if s.webdav_password.starts_with(crate::security::DPAPI_PREFIX) {
                            match crate::security::decrypt_secret(&s.webdav_password) {
                                Ok(pw) => s.webdav_password = pw,
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to decrypt stored WebDAV password; cleared: {}",
                                        e
                                    );
                                    s.webdav_password = String::new();
                                }
                            }
                        }
                        settings = s;
                    }
                    Err(e) => {
                        // Fail loudly instead of silently resetting every setting:
                        // a corrupt settings blob must not masquerade as "defaults".
                        tracing::error!(
                            "Corrupt app_settings JSON; falling back to defaults: {}",
                            e
                        );
                    }
                },
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(e) => {
                    tracing::warn!("Failed to read app_settings: {}", e);
                }
            }
        }

        let arc = Arc::new(settings);
        *self.settings_cache.write() = Some(Arc::clone(&arc));
        Ok(arc)
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), SettingsError> {
        // Serialize a copy whose WebDAV password is DPAPI-encrypted at rest.
        // The in-memory cache keeps the plaintext form (as the frontend sees
        // it), so a get_settings round-trip must never hand back the cipher.
        // A serialize failure must not write "" — the next load would silently
        // reset every setting to defaults. Fail loud instead.
        let mut for_json = settings.clone();
        if !for_json.webdav_password.is_empty() {
            for_json.webdav_password = crate::security::encrypt_secret(&for_json.webdav_password)
                .map_err(SettingsError::Encryption)?;
        }
        let json = serde_json::to_string(&for_json)?;
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
                .collect::<SqlResult<Vec<_>>>()?;
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
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).to_rfc3339();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 1 AND updated_at < ?",
            )?;
            let ids = stmt
                .query_map([&cutoff], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
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
