//! Import (with hash merge) and full-content export paging.
use rusqlite::{params, Error as SqlError, Result as SqlResult};

use super::{ClipboardDb, ALIAS_MAX_CHARS};
use crate::detect::detect_sensitive;
use crate::security;
use crate::{ClipboardRecord, Settings};

#[derive(Debug, Clone)]
pub struct ExportCursor {
    pub is_pinned: bool,
    pub updated_at: String,
    pub id: i64,
}

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
                            THEN ? ELSE auto_expire_at END
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
                    if super::ClipboardDb::set_record_tags_by_name_conn(&tx, id, &record.tags)? {
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
                    content, content_type, source_app, source_window, source_name, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html, alias, content_len
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.content,
                    content_type,
                    record.source_app,
                    record.source_window,
                    record.source_name,
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
                if super::ClipboardDb::set_record_tags_by_name_conn(&tx, record_id, &record.tags)? {
                    tags_changed += 1;
                }
            }
            imported += 1;
        }

        let overflow_media = self.evict_over_limit(&tx, max_records)?;
        tx.commit()?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);
        Ok((imported, merged, tags_changed))
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
        for record in &mut records {
            record.media_abs = None;
            record.thumb_abs = None;
        }
        Ok(records)
    }

    pub fn get_records_for_export_page(
        &self,
        limit: i32,
        cursor: Option<&ExportCursor>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut sql = format!(
            "SELECT {} FROM records WHERE is_trashed = 0",
            super::RECORD_COLS
        );
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(cursor) = cursor {
            sql.push_str(
                " AND (is_pinned < ? OR (is_pinned = ? AND (updated_at < ? OR (updated_at = ? AND id < ?))))",
            );
            values.push(Box::new(cursor.is_pinned as i32));
            values.push(Box::new(cursor.is_pinned as i32));
            values.push(Box::new(cursor.updated_at.clone()));
            values.push(Box::new(cursor.updated_at.clone()));
            values.push(Box::new(cursor.id));
        }
        sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC, id DESC LIMIT ?");
        values.push(Box::new(limit.max(1)));
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|value| value.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(refs.as_slice(), |row| self.map_record_row(row))?
            .collect::<SqlResult<Vec<_>>>()?;
        self.enrich_tags(&conn, &mut records, true)?;
        for record in &mut records {
            record.media_abs = None;
            record.thumb_abs = None;
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;
    use crate::ClipboardRecord;
    use std::path::PathBuf;

    fn temp_db() -> (ClipboardDb, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "clipvault_import_tag_test_{}_{}",
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

    fn make_record(content: &str, hash: &str, tags: &[&str]) -> ClipboardRecord {
        let now = chrono::Utc::now().to_rfc3339();
        ClipboardRecord {
            id: 0,
            content: content.to_string(),
            content_type: "text".into(),
            source_app: String::new(),
            source_window: String::new(),
            source_name: String::new(),
            hash: hash.to_string(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: now.clone(),
            updated_at: now,
            tags: tags.iter().map(|s| s.to_string()).collect(),
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
    fn import_creates_tags_and_links() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(
            &[make_record("hello", "hash-1", &["重要", "链接"])],
            100,
            None,
        )
        .unwrap();
        let exported = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(exported.len(), 1);
        let mut tags = exported[0].tags.clone();
        tags.sort();
        let mut want = vec!["链接".to_string(), "重要".to_string()];
        want.sort();
        assert_eq!(tags, want);
        cleanup(dir);
    }

    #[test]
    fn import_merge_replaces_tags_when_incoming_has_tags() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(&[make_record("same", "hash-x", &["重要"])], 100, None)
            .unwrap();
        db.import_records_with_merge(&[make_record("same", "hash-x", &["链接"])], 100, None)
            .unwrap();
        let exported = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].tags, ["链接"]);
        cleanup(dir);
    }

    #[test]
    fn import_merge_preserves_local_tags_for_tagless_snapshot() {
        let (db, dir) = temp_db();
        db.import_records_with_merge(&[make_record("same", "hash-y", &["重要"])], 100, None)
            .unwrap();
        db.import_records_with_merge(&[make_record("same", "hash-y", &[])], 100, None)
            .unwrap();
        let exported = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(exported[0].tags, ["重要"]);
        cleanup(dir);
    }

    #[test]
    fn tags_changed_counts_only_real_changes() {
        let (db, dir) = temp_db();
        // New record with tags → counts 1.
        let (_, _, tc) = db
            .import_records_with_merge(&[make_record("a", "hash-tc", &["重要"])], 100, None)
            .unwrap();
        assert_eq!(tc, 1);
        // Merge with identical tags → 0 (no spurious count).
        let (_, _, tc) = db
            .import_records_with_merge(&[make_record("a", "hash-tc", &["重要"])], 100, None)
            .unwrap();
        assert_eq!(tc, 0);
        // Merge with a changed tag set → 1.
        let (_, _, tc) = db
            .import_records_with_merge(&[make_record("a", "hash-tc", &["链接"])], 100, None)
            .unwrap();
        assert_eq!(tc, 1);
        // Merge with empty tags → 0 (preserves local, counts nothing).
        let (_, _, tc) = db
            .import_records_with_merge(&[make_record("a", "hash-tc", &[])], 100, None)
            .unwrap();
        assert_eq!(tc, 0);
        cleanup(dir);
    }

    #[test]
    fn import_deduplicates_repeated_hashes_in_one_batch() {
        let (db, dir) = temp_db();
        let records = [
            make_record("same", "batch-duplicate", &[]),
            make_record("same", "batch-duplicate", &[]),
        ];

        let (imported, merged, _) = db.import_records_with_merge(&records, 100, None).unwrap();

        assert_eq!((imported, merged), (1, 1));
        assert_eq!(db.get_records_for_export(10, 0).unwrap().len(), 1);
        cleanup(dir);
    }

    #[test]
    fn import_rejects_oversized_content() {
        let record = make_record(
            &"x".repeat(super::MAX_IMPORT_CONTENT_BYTES + 1),
            "oversized-content",
            &[],
        );

        let error = super::validate_import_records(&[record]).unwrap_err();

        assert!(error.contains("正文过大"));
    }

    #[test]
    fn export_cursor_pages_without_offset() {
        let (db, dir) = temp_db();
        let records = [
            make_record("first", "cursor-1", &[]),
            make_record("second", "cursor-2", &[]),
        ];
        db.import_records_with_merge(&records, 100, None).unwrap();

        let first = db.get_records_for_export_page(1, None).unwrap();
        let cursor = super::ExportCursor {
            is_pinned: first[0].is_pinned,
            updated_at: first[0].updated_at.clone(),
            id: first[0].id,
        };
        let second = db.get_records_for_export_page(1, Some(&cursor)).unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].id, second[0].id);
        cleanup(dir);
    }

    #[test]
    fn import_sanitize_recomputes_expiry_and_rechecks_sensitive() {
        let (db, dir) = temp_db();
        let mut rec = make_record("Your verification code: 123456", "sanitize-1", &[]);
        // Hostile/stale bundle: marks content non-sensitive and carries a past
        // expiry. With sanitization enabled neither may survive the import.
        rec.is_sensitive = false;
        rec.auto_expire_at = Some("2020-01-01T00:00:00Z".into());
        let policy = super::ImportSanitize {
            recheck_sensitive: true,
            sensitive_auto_expire_seconds: 600,
        };

        db.import_records_with_merge(&[rec], 100, Some(policy))
            .unwrap();

        let rows = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_sensitive, "detection must re-flag the record");
        let expiry = rows[0].auto_expire_at.as_deref().expect("expiry set");
        assert!(
            expiry > "2026-01-01T00:00:00Z",
            "expiry must be recomputed from now, got {expiry}"
        );
        cleanup(dir);
    }

    #[test]
    fn import_sanitize_never_downgrades_sensitive_flag() {
        let (db, dir) = temp_db();
        let mut rec = make_record("plain text", "sanitize-2", &[]);
        rec.is_sensitive = true; // remote says sensitive even though text is plain
        let policy = super::ImportSanitize {
            recheck_sensitive: true,
            sensitive_auto_expire_seconds: 600,
        };

        db.import_records_with_merge(&[rec], 100, Some(policy))
            .unwrap();

        let rows = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_sensitive);
        assert!(rows[0].auto_expire_at.is_some());
        cleanup(dir);
    }

    #[test]
    fn import_sanitize_preserves_past_expiry_when_disabled() {
        let (db, dir) = temp_db();
        let mut rec = make_record("plain text", "sanitize-3", &[]);
        rec.is_sensitive = true;
        rec.auto_expire_at = Some("2020-01-01T00:00:00Z".into());

        // Legacy callers (no policy) keep the previous passthrough behaviour.
        db.import_records_with_merge(&[rec], 100, None).unwrap();

        let rows = db.get_records_for_export(10, 0).unwrap();
        assert_eq!(
            rows[0].auto_expire_at.as_deref(),
            Some("2020-01-01T00:00:00Z")
        );
        cleanup(dir);
    }
}
