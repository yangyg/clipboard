//! Import with hash merge + bundle validation.
//! Export paging lives in `records_export.rs`.
use rusqlite::{params, Error as SqlError, Result as SqlResult};

use super::{ClipboardDb, ALIAS_MAX_CHARS};
use crate::detect::detect_sensitive;
use crate::security;
use crate::{ClipboardRecord, Settings};

pub const MAX_IMPORT_RECORDS: usize = 100_000;
pub const MAX_IMPORT_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_IMPORT_CONTENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMPORT_HTML_BYTES: usize = 512 * 1024;

/// Sanitization policy applied to untrusted record bundles (JSON import /
/// WebDAV pull). Mirrors the capture pipeline so remote `is_sensitive` /
/// `auto_expire_at` metadata cannot silently delete or mislabel local data.
#[derive(Debug, Clone, Copy)]
pub struct ImportSanitize {
    /// Re-run `detect_sensitive` on incoming text so a bundle that flips
    /// `is_sensitive=false` cannot bypass the local privacy filter.
    pub recheck_sensitive: bool,
    /// Fresh TTL applied to sensitive rows. Overrides the remote expiry,
    /// which may already be in the past and would otherwise be hard-deleted
    /// by the next cleanup sweep immediately after import.
    pub sensitive_auto_expire_seconds: i32,
}

impl From<&Settings> for ImportSanitize {
    fn from(s: &Settings) -> Self {
        Self {
            recheck_sensitive: s.enable_sensitive_detection,
            sensitive_auto_expire_seconds: s.sensitive_auto_expire_seconds,
        }
    }
}

pub fn validate_import_records(records: &[ClipboardRecord]) -> Result<(), String> {
    if records.len() > MAX_IMPORT_RECORDS {
        return Err(format!("导入记录过多（上限 {} 条）", MAX_IMPORT_RECORDS));
    }

    let mut total_bytes = 0usize;
    for record in records {
        if record.content.len() > MAX_IMPORT_CONTENT_BYTES {
            return Err(format!(
                "记录正文过大（单条上限 {} MB）",
                MAX_IMPORT_CONTENT_BYTES / (1024 * 1024)
            ));
        }
        if let Some(html) = record.content_html.as_deref() {
            if html.len() > MAX_IMPORT_HTML_BYTES {
                return Err("记录 HTML 过大（单条上限 512 KB）".into());
            }
            total_bytes = total_bytes.saturating_add(html.len());
        }
        total_bytes = total_bytes.saturating_add(record.content.len());
        if total_bytes > MAX_IMPORT_TOTAL_BYTES {
            return Err("导入内容过大（总上限 64 MB）".into());
        }
    }
    Ok(())
}

fn validation_error(message: String) -> SqlError {
    SqlError::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    )))
}

impl ClipboardDb {
    pub fn import_records(
        &self,
        records: &[ClipboardRecord],
        max_records: i32,
        sanitize: Option<ImportSanitize>,
    ) -> SqlResult<i32> {
        let (imported, _, _) = self.import_records_with_merge(records, max_records, sanitize)?;
        Ok(imported)
    }

    /// Import with hash dedup. Existing hashes get a shallow merge:
    /// newer `updated_at`, OR on favorite/pin, max `copy_count`, fill missing media paths.
    /// Returns `(inserted, merged, tags_changed)` — `tags_changed` counts records whose
    /// tag links were actually written/changed (WebDAV pull surfaces this in its summary).
    pub fn import_records_with_merge(
        &self,
        records: &[ClipboardRecord],
        max_records: i32,
        sanitize: Option<ImportSanitize>,
    ) -> SqlResult<(i32, i32, i32)> {
        validate_import_records(records).map_err(validation_error)?;
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut imported = 0;
        let mut merged = 0;
        let mut tags_changed = 0;
        // Batch tag-id cache + deferred FTS rebuilds: per-record tag work drops
        // from ~6-8 queries (ensure tag ×N + FTS refresh) to ~3, and FTS is
        // rebuilt once for the whole batch.
        let mut tag_id_cache: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        let mut fts_dirty: Vec<i64> = Vec::new();

        // Batch-load existing hashes in one query instead of per-record lookups.
        // Active rows only: importing a hash that exists *only in the trash* must
        // insert a fresh active record (resurrect) rather than silently merge into
        // the trashed row (which the capture-dedup path also treats as not present).
        let mut existing_hashes: std::collections::HashSet<String> = {
            let mut stmt = tx.prepare("SELECT hash FROM records WHERE is_trashed = 0")?;
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

            // Normalize legacy hashes: old builds baked CF_HTML bytes into the
            // hash, so a legacy bundle could re-insert duplicates of local rows.
            // Text identity is sha256(sha256(content)) now (see migrate_text_hash_v2);
            // images keep their pixel hash untouched.
            let hash: String = if is_image {
                record.hash.clone()
            } else {
                crate::detect::sha256_hash(&crate::detect::sha256_hash(&record.content))
            };

            // Boundary sanitization: `is_sensitive` / `auto_expire_at` come from
            // an untrusted bundle. Never downgrade sensitivity; recompute any
            // expiry from *now* so a past remote TTL cannot delete imported
            // data moments after it lands.
            let (is_sensitive, auto_expire_at) = match sanitize {
                Some(policy) => {
                    let sensitive = record.is_sensitive
                        || (policy.recheck_sensitive
                            && !is_image
                            && detect_sensitive(&record.content));
                    let expire = if sensitive && policy.sensitive_auto_expire_seconds > 0 {
                        Some(
                            (chrono::Utc::now()
                                + chrono::Duration::seconds(
                                    policy.sensitive_auto_expire_seconds as i64,
                                ))
                            .to_rfc3339(),
                        )
                    } else {
                        None
                    };
                    (sensitive, expire)
                }
                None => (record.is_sensitive, record.auto_expire_at.clone()),
            };

            if existing_hashes.contains(&hash) {
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
                            THEN ? ELSE thumb_path END,
                        is_sensitive = CASE WHEN is_sensitive = 1 OR ? = 1 THEN 1 ELSE is_sensitive END,
                        auto_expire_at = CASE
                            WHEN (is_sensitive = 1 OR ? = 1)
                                 AND (auto_expire_at IS NULL OR auto_expire_at = '')
                                 AND ? IS NOT NULL
                            THEN ? ELSE auto_expire_at END,
                        -- First-origin semantics: adopt the incoming device only
                        -- when it is non-empty AND the earlier creator (or the
                        -- local row has no origin yet). A non-empty origin is
                        -- never overwritten or erased.
                        source_device_id = CASE
                            WHEN ? != '' AND (source_device_id = '' OR ? < created_at)
                            THEN ? ELSE source_device_id END
                     WHERE hash = ? AND is_trashed = 0",
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
                        is_sensitive as i32,
                        auto_expire_at,
                        auto_expire_at,
                        auto_expire_at,
                        record.source_device_id,
                        record.created_at,
                        record.source_device_id,
                        hash,
                    ],
                )?;
                if changed > 0 {
                    merged += 1;
                }
                // Tag sync: replace the links only when the incoming snapshot
                // actually carries tags — a bundle written before tag-sync
                // shipped has an empty `tags` array and must not wipe the
                // local associations.
                if record.tags.iter().any(|t| !t.trim().is_empty()) {
                    // Same predicate as the UPDATE above: a trashed row may
                    // share the hash with an active row, and tags belong to
                    // the active one.
                    let id: i64 = tx.query_row(
                        "SELECT id FROM records WHERE hash = ? AND is_trashed = 0",
                        [&hash],
                        |row| row.get(0),
                    )?;
                    if super::ClipboardDb::set_record_tags_by_name_conn_cached(
                        &tx,
                        id,
                        &record.tags,
                        &mut tag_id_cache,
                        &mut fts_dirty,
                    )? {
                        tags_changed += 1;
                    }
                }
                continue;
            }

            let mut alias = record.alias.trim().to_string();
            if alias.chars().count() > ALIAS_MAX_CHARS {
                alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
            }

            tx.execute(
                "INSERT INTO records (
                    content, content_type, source_app, source_window, source_name, source_device_id, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html, alias, content_len
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.content,
                    content_type,
                    record.source_app,
                    record.source_window,
                    record.source_name,
                    record.source_device_id,
                    hash,
                    record.copy_count,
                    record.is_favorite as i32,
                    record.is_pinned as i32,
                    is_sensitive as i32,
                    record.is_trashed as i32,
                    auto_expire_at,
                    record.created_at,
                    record.updated_at,
                    media_path,
                    thumb_path,
                    record.width,
                    record.height,
                    content_html,
                    alias,
                    record.content.chars().count() as i64,
                ],
            )?;
            existing_hashes.insert(hash);
            if record.tags.iter().any(|t| !t.trim().is_empty()) {
                let record_id = tx.last_insert_rowid();
                if super::ClipboardDb::set_record_tags_by_name_conn_cached(
                    &tx,
                    record_id,
                    &record.tags,
                    &mut tag_id_cache,
                    &mut fts_dirty,
                )? {
                    tags_changed += 1;
                }
            }
            imported += 1;
        }

        // One batched FTS rebuild for every record whose tags actually changed
        // (replaces per-record refresh_record_fts).
        if !fts_dirty.is_empty() {
            fts_dirty.sort_unstable();
            fts_dirty.dedup();
            let placeholders = Self::id_placeholders(fts_dirty.len());
            let params: Vec<&dyn rusqlite::types::ToSql> = fts_dirty
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            tx.execute(
                &format!("DELETE FROM records_fts WHERE rowid IN ({placeholders})"),
                params.as_slice(),
            )?;
            tx.execute(
                &format!(
                    "INSERT INTO records_fts(rowid, content, source_app, source_window, tags, alias)
                     SELECT
                        r.id,
                        {},
                        r.source_app,
                        r.source_window,
                        COALESCE((
                            SELECT group_concat(t.name, ' ')
                            FROM record_tags rt
                            INNER JOIN tags t ON t.id = rt.tag_id
                            WHERE rt.record_id = r.id
                        ), ''),
                        r.alias
                     FROM records r WHERE r.id IN ({placeholders})",
                    Self::fts_content_sql()
                ),
                params.as_slice(),
            )?;
        }

        let overflow_media = self.evict_over_limit(&tx, max_records)?;
        tx.commit()?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);
        Ok((imported, merged, tags_changed))
    }
}
