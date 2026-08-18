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
    #[error("stored settings blob is corrupt; refusing to overwrite it")]
    CorruptStoredBlob,
}

/// Resolve the at-rest representation for one secret. Reuses the previously
/// stored ciphertext when the plaintext is unchanged (DPAPI-encrypted values
/// only — legacy plaintext is re-encrypted, upgrading it to encryption). An
/// empty plaintext clears the secret.
fn resolve_stored_secret(
    plaintext: &str,
    cached_plain: &str,
    cached_raw: &str,
) -> Result<String, SettingsError> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    if !cached_plain.is_empty() && cached_plain == plaintext && !cached_raw.is_empty() {
        return Ok(cached_raw.to_string());
    }
    crate::security::encrypt_secret(plaintext).map_err(SettingsError::Encryption)
}

impl ClipboardDb {
    /// Decrypt a DPAPI-encrypted secret in place, keeping the stored ciphertext
    /// when decryption fails (a later save would otherwise overwrite the only
    /// remaining copy with an empty string). Legacy plaintext values (no
    /// prefix) pass through unchanged.
    fn decrypt_secret_or_keep(secret: &mut String, label: &str) {
        if !secret.starts_with(crate::security::DPAPI_PREFIX) {
            return;
        }
        match crate::security::decrypt_secret(secret) {
            Ok(plain) => *secret = plain,
            Err(e) => {
                tracing::error!(
                    "Failed to decrypt stored {label}; keeping the stored value \
                     (a later save would otherwise overwrite it with an empty string): {e}"
                );
                // Keep the ciphertext as loaded; the settings UI will show it
                // until the user re-enters the value. The encryption guard in
                // `save_settings` must never double-encrypt it.
            }
        }
    }

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
                        // Remember the raw stored forms so save_settings can
                        // reuse unchanged ciphertext instead of re-encrypting.
                        let stored_password = s.webdav_password.clone();
                        let stored_api_key = s.ai_api_key.clone();
                        Self::decrypt_secret_or_keep(&mut s.webdav_password, "WebDAV password");
                        Self::decrypt_secret_or_keep(&mut s.ai_api_key, "AI API key");
                        let pw_pair = if stored_password.starts_with(crate::security::DPAPI_PREFIX)
                        {
                            (s.webdav_password.clone(), stored_password)
                        } else {
                            (String::new(), String::new())
                        };
                        let key_pair = if stored_api_key.starts_with(crate::security::DPAPI_PREFIX)
                        {
                            (s.ai_api_key.clone(), stored_api_key)
                        } else {
                            (String::new(), String::new())
                        };
                        *self.secrets_cache.lock() =
                            Some((pw_pair.0, pw_pair.1, key_pair.0, key_pair.1));
                        settings = s;
                    }
                    Err(e) => {
                        // Fail loudly instead of silently resetting every setting:
                        // a corrupt blob masquerading as "defaults" would be
                        // overwritten by the next save (incl. WebDAV
                        // persist_last_sync), destroying the original data.
                        tracing::error!("Corrupt app_settings JSON: {}", e);
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ));
                    }
                },
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(e) => return Err(e),
            }
        }

        settings.normalize_ai_models();
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
        {
            let (cached_pw_plain, cached_pw_raw, cached_key_plain, cached_key_raw) = self
                .secrets_cache
                .lock()
                .as_ref()
                .cloned()
                .unwrap_or_default();
            let new_password =
                resolve_stored_secret(&for_json.webdav_password, &cached_pw_plain, &cached_pw_raw)?;
            let new_api_key =
                resolve_stored_secret(&for_json.ai_api_key, &cached_key_plain, &cached_key_raw)?;
            *self.secrets_cache.lock() = Some((
                if new_password.is_empty() {
                    String::new()
                } else {
                    for_json.webdav_password.clone()
                },
                new_password.clone(),
                if new_api_key.is_empty() {
                    String::new()
                } else {
                    for_json.ai_api_key.clone()
                },
                new_api_key.clone(),
            ));
            for_json.webdav_password = new_password;
            for_json.ai_api_key = new_api_key;
        }
        let json = serde_json::to_string(&for_json)?;
        // Refuse to overwrite a stored blob we never successfully loaded: a
        // save built from defaults (e.g. WebDAV persist_last_sync) would
        // silently destroy the corrupt-but-recoverable original data.
        if self.settings_cache.read().is_none() {
            let stored = self
                .lock_read()
                .query_row::<String, _, _>(
                    "SELECT value FROM settings WHERE key = 'app_settings'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            if let Some(blob) = stored {
                if serde_json::from_str::<Settings>(&blob).is_err() {
                    return Err(SettingsError::CorruptStoredBlob);
                }
            }
        }
        {
            let conn = self.lock_write();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app_settings', ?)",
                [&json],
            )?;
        }
        *self.settings_cache.write() = Some(Arc::new(settings.clone()));
        Ok(())
    }

    /// Persist only the window size remembered after a user resize.
    /// Other fields are read-modify-written so a concurrent user save is kept.
    pub fn save_window_geometry(&self, width: i32, height: i32) -> Result<(), SettingsError> {
        let current = self.get_settings()?;
        if current.window_width == width && current.window_height == height {
            return Ok(());
        }
        let mut next = (*current).clone();
        next.window_width = width;
        next.window_height = height;
        self.save_settings(&next)
    }

    /// Persist WebDAV sync stamps / device-name map without OS side effects.
    /// Overlays onto a fresh load so a concurrent user save (theme, etc.) is kept.
    pub fn save_sync_metadata(&self, patch: &Settings) -> Result<Settings, SettingsError> {
        let mut next = (*self.get_settings()?).clone();
        next.webdav_last_sync_at = patch.webdav_last_sync_at.clone();
        next.webdav_device_names = patch.webdav_device_names.clone();
        next.webdav_device_id = patch.webdav_device_id.clone();
        next.webdav_device_name = patch.webdav_device_name.clone();
        self.save_settings(&next)?;
        Ok(next)
    }

    /// Fill empty `webdav_device_id` / `webdav_device_name` once at startup.
    pub fn ensure_device_identity(&self) -> Result<(), SettingsError> {
        let current = self.get_settings()?;
        let mut next = (*current).clone();
        let mut changed = false;
        if next.webdav_device_id.trim().is_empty() {
            next.webdav_device_id = uuid::Uuid::new_v4().to_string();
            changed = true;
        }
        if next.webdav_device_name.trim().is_empty() {
            next.webdav_device_name =
                std::env::var("COMPUTERNAME").unwrap_or_else(|_| "My Device".to_string());
            changed = true;
        }
        if changed {
            self.save_settings(&next)?;
            tracing::info!("Device identity ensured (id={})", next.webdav_device_id);
        }
        Ok(())
    }

    pub fn cleanup_expired(&self) -> SqlResult<Vec<i64>> {
        let conn = self.lock_write();
        let now = chrono::Utc::now().to_rfc3339();
        // Respect pin/favorite, consistent with retention and max-record
        // eviction: a user who pinned/favorited a sensitive record (e.g. an OTP
        // kept for later) must not have it hard-deleted at expiry.
        // Trashed rows are also excluded — once a record is in the trash,
        // `cleanup_retention` (the trash-retention window) owns its lifecycle,
        // and a trashed sensitive record must stay recoverable.
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?
                 AND is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0",
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
            "DELETE FROM records WHERE auto_expire_at IS NOT NULL AND auto_expire_at <= ?
             AND is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0",
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
        let conn = self.lock_write();
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

#[cfg(test)]
mod tests {
    use super::ClipboardDb;
    use crate::ClipboardRecord;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        crate::db::test_util::temp_db("settings")
    }

    fn cleanup(dir: PathBuf) {
        crate::db::test_util::cleanup(dir)
    }

    #[test]
    fn save_window_geometry_only_updates_width_and_height() {
        let (db, dir) = temp_db();
        let mut settings = (*db.get_settings().unwrap()).clone();
        settings.theme = "oled".into();
        settings.webdav_device_id = "keep-me".into();
        settings.window_width = 800;
        settings.window_height = 600;
        db.save_settings(&settings).unwrap();

        db.save_window_geometry(1024, 768).unwrap();

        let saved = db.get_settings().unwrap();
        assert_eq!(saved.window_width, 1024);
        assert_eq!(saved.window_height, 768);
        assert_eq!(saved.theme, "oled");
        assert_eq!(saved.webdav_device_id, "keep-me");
        cleanup(dir);
    }

    #[test]
    fn save_sync_metadata_overlays_sync_fields_only() {
        let (db, dir) = temp_db();
        let mut settings = (*db.get_settings().unwrap()).clone();
        settings.theme = "nord".into();
        settings.webdav_device_id = "dev-1".into();
        db.save_settings(&settings).unwrap();

        let mut patch = (*db.get_settings().unwrap()).clone();
        patch.theme = "should-not-stick".into();
        patch.webdav_last_sync_at = Some("2026-01-01T00:00:00Z".into());
        patch.webdav_device_name = "Office PC".into();
        let saved = db.save_sync_metadata(&patch).unwrap();
        assert_eq!(saved.theme, "nord");
        assert_eq!(
            saved.webdav_last_sync_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(saved.webdav_device_name, "Office PC");
        assert_eq!(saved.webdav_device_id, "dev-1");
        cleanup(dir);
    }

    fn make_record(content: &str, hash: &str, auto_expire_at: Option<&str>) -> ClipboardRecord {
        let now = chrono::Utc::now().to_rfc3339();
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
            is_sensitive: true,
            is_trashed: false,
            auto_expire_at: auto_expire_at.map(str::to_string),
            created_at: now.clone(),
            updated_at: now,
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

    #[test]
    fn cleanup_expired_skips_trashed_records() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record(
                "expired-trash",
                "exp-trash-1",
                Some("2000-01-01T00:00:00Z"),
            )],
            100,
            None,
        )
        .unwrap();
        let id = db.get_records_for_export(10, 0).unwrap()[0].id;
        db.trash_record(id).unwrap();

        // Simulate a row that somehow still carries a past expiry while trashed
        // (e.g. a legacy DB or a future code path that restores it).
        {
            let conn = db.lock_write();
            conn.execute(
                "UPDATE records SET auto_expire_at = '2000-01-01T00:00:00Z' WHERE id = ?",
                [id],
            )
            .unwrap();
        }

        let expired = db.cleanup_expired().unwrap();
        assert!(
            expired.is_empty(),
            "trashed rows must survive cleanup_expired"
        );
        assert_eq!(db.get_trash_count().unwrap(), 1);
        cleanup(dir);
    }

    #[test]
    fn cleanup_expired_still_deletes_active_expired_rows() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record(
                "expired-active",
                "exp-active-1",
                Some("2000-01-01T00:00:00Z"),
            )],
            100,
            None,
        )
        .unwrap();

        let expired = db.cleanup_expired().unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(db.get_records_for_export(10, 0).unwrap().len(), 0);
        cleanup(dir);
    }

    #[test]
    fn trash_record_clears_auto_expire() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record(
                "expiring",
                "exp-clear-1",
                Some("2030-01-01T00:00:00Z"),
            )],
            100,
            None,
        )
        .unwrap();
        let id = db.get_records_for_export(10, 0).unwrap()[0].id;

        db.trash_record(id).unwrap();

        let conn = db.lock_write();
        let expiry: Option<String> = conn
            .query_row(
                "SELECT auto_expire_at FROM records WHERE id = ?",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            expiry.is_none(),
            "trash must clear the sensitive auto-expiry"
        );
        cleanup(dir);
    }
}
