//! Import (with hash merge) and full-content export paging.
use rusqlite::{params, Result as SqlResult};

use super::{ClipboardDb, ALIAS_MAX_CHARS};
use crate::security;
use crate::ClipboardRecord;

impl ClipboardDb {
    pub fn import_records(&self, records: &[ClipboardRecord], max_records: i32) -> SqlResult<i32> {
        let (imported, _) = self.import_records_with_merge(records, max_records)?;
        Ok(imported)
    }

    /// Import with hash dedup. Existing hashes get a shallow merge:
    /// newer `updated_at`, OR on favorite/pin, max `copy_count`, fill missing media paths.
    /// Returns `(inserted, merged)`.
    pub fn import_records_with_merge(
        &self,
        records: &[ClipboardRecord],
        max_records: i32,
    ) -> SqlResult<(i32, i32)> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut imported = 0;
        let mut merged = 0;

        // Batch-load existing hashes in one query instead of per-record lookups.
        let existing_hashes: std::collections::HashSet<String> = {
            let mut stmt = tx.prepare("SELECT hash FROM records")?;
            let hashes: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            hashes.into_iter().collect()
        };

        for record in records {
            // M-5: Avoid cloning the entire record (content + content_html can be large).
            // Only create local overrides for fields we might sanitize.
            let mut content_type = security::normalize_content_type(&record.content_type);
            if content_type == "link" && !security::is_openable_link(&record.content) {
                content_type = "text".into();
            }
            let mut media_path = record.media_path.as_deref();
            let mut thumb_path = record.thumb_path.as_deref();
            if let Some(mp) = media_path {
                if !security::is_allowed_media_rel(mp) {
                    media_path = None;
                    thumb_path = None;
                }
            }
            if let Some(tp) = thumb_path {
                if !security::is_allowed_media_rel(tp) {
                    thumb_path = None;
                }
            }
            // Cap HTML blob size from malicious imports
            let content_html = record
                .content_html
                .as_deref()
                .filter(|h| h.len() <= 512 * 1024)
                // Untrusted boundary: drop HTML that could execute on re-paste
                // into a rich-text editor (import + WebDAV share this path).
                .filter(|h| security::is_safe_import_html(h));

            // Skip empty text records; image records may have empty content with media_path
            let is_image = content_type == "image";
            if (!is_image && record.content.trim().is_empty()) || record.hash.trim().is_empty() {
                continue;
            }

            if existing_hashes.contains(&record.hash) {
                let changed = tx.execute(
                    "UPDATE records SET
                        is_favorite = CASE WHEN is_favorite = 1 OR ? = 1 THEN 1 ELSE 0 END,
                        is_pinned = CASE WHEN is_pinned = 1 OR ? = 1 THEN 1 ELSE 0 END,
                        copy_count = CASE WHEN copy_count < ? THEN ? ELSE copy_count END,
                        updated_at = CASE WHEN updated_at < ? THEN ? ELSE updated_at END,
                        media_path = CASE
                            WHEN (media_path IS NULL OR media_path = '') AND ? IS NOT NULL AND ? != ''
                            THEN ? ELSE media_path END,
                        thumb_path = CASE
                            WHEN (thumb_path IS NULL OR thumb_path = '') AND ? IS NOT NULL AND ? != ''
                            THEN ? ELSE thumb_path END
                     WHERE hash = ?",
                    params![
                        record.is_favorite as i32,
                        record.is_pinned as i32,
                        record.copy_count,
                        record.copy_count,
                        record.updated_at,
                        record.updated_at,
                        media_path,
                        media_path,
                        media_path,
                        thumb_path,
                        thumb_path,
                        thumb_path,
                        record.hash,
                    ],
                )?;
                if changed > 0 {
                    merged += 1;
                }
                continue;
            }

            let mut alias = record.alias.trim().to_string();
            if alias.chars().count() > ALIAS_MAX_CHARS {
                alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
            }

            tx.execute(
                "INSERT INTO records (
                    content, content_type, source_app, source_window, source_name, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html, alias
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.content,
                    content_type,
                    record.source_app,
                    record.source_window,
                    record.source_name,
                    record.hash,
                    record.copy_count,
                    record.is_favorite as i32,
                    record.is_pinned as i32,
                    record.is_sensitive as i32,
                    record.is_trashed as i32,
                    record.auto_expire_at,
                    record.created_at,
                    record.updated_at,
                    media_path,
                    thumb_path,
                    record.width,
                    record.height,
                    content_html,
                    alias,
                ],
            )?;
            imported += 1;
        }

        let overflow_media = self.evict_over_limit(&tx, max_records)?;
        tx.commit()?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);
        Ok((imported, merged))
    }

    /// Full-content page for export/backup (never use list truncation columns).
    pub fn get_records_for_export(
        &self,
        limit: i32,
        offset: i32,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE is_trashed = 0
             ORDER BY is_pinned DESC, updated_at DESC LIMIT ? OFFSET ?",
            super::RECORD_COLS
        ))?;
        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(params![limit, offset], |row| self.map_record_row(row))?
            .collect::<SqlResult<Vec<_>>>()?;
        self.enrich_tags(&conn, &mut records, true)?;
        Ok(records)
    }
}
