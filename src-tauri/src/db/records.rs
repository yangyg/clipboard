//! Record CRUD, search, trash, favorites, import/export.
use rusqlite::{params, Connection, Result as SqlResult, Row};

use super::{ClipboardDb, ImageMeta, ContentType, RECORD_COLS, RECORD_COLS_LIST, ALIAS_MAX_CHARS};
use crate::media;
use crate::ClipboardRecord;

impl ClipboardDb {
    // === Query helpers ===

    /// Escape `%`, `_`, `\` for use with `LIKE … ESCAPE '\'`.
    /// Kept for unit tests; production short search uses `instr` (no LIKE wildcards).
    #[cfg(test)]
    pub(super) fn escape_like(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    }

    /// FTS5 trigram: substring MATCH needs ≥3 chars. Returns quoted token.
    pub(super) fn build_fts_match(query: &str) -> Option<String> {
        let q = query.trim();
        if q.chars().count() < 3 {
            return None;
        }
        Some(format!("\"{}\"", q.replace('"', "\"\"")))
    }

    /// Short (1–2 char) search: one `instr` pass over records (+ optional tag EXISTS).
    /// Avoids leading-wildcard `LIKE '%X%'` which cannot use indexes and multiplies scans.
    pub(super) fn push_short_query_predicate(
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
        query: &str,
        include_tags: bool,
    ) {
        sql.push_str(
            "instr(content, ?) > 0
             OR instr(alias, ?) > 0
             OR instr(source_app, ?) > 0
             OR instr(source_window, ?) > 0",
        );
        let q = query.to_string();
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        params.push(Box::new(q.clone()));
        if include_tags {
            sql.push_str(
                " OR EXISTS (
                    SELECT 1 FROM record_tags rt
                    INNER JOIN tags t ON t.id = rt.tag_id
                    WHERE rt.record_id = records.id AND instr(t.name, ?) > 0
                 )",
            );
            params.push(Box::new(q));
        }
    }

    /// FTS MATCH token. When `include_tags` is false, limit columns so tag names are not searchable.
    pub(super) fn build_fts_match_expr(query: &str, include_tags: bool) -> Option<String> {
        let token = Self::build_fts_match(query)?;
        if include_tags {
            Some(token)
        } else {
            // Column filter excludes the FTS `tags` column.
            Some(format!(
                "{{content alias source_app source_window}}: {token}"
            ))
        }
    }

    /// Whitelist sort keys → ORDER BY fragment. Unknown values fall back to updated_desc.
    /// Non-trash lists keep pinned rows first.
    pub(super) fn order_by_clause(trashed: bool, sort: Option<&str>) -> &'static str {
        let secondary = match sort.unwrap_or("updated_desc") {
            "updated_asc" => "updated_at ASC",
            "created_desc" => "created_at DESC",
            "copies_desc" => "copy_count DESC, updated_at DESC",
            _ => "updated_at DESC",
        };
        if trashed {
            return secondary;
        }
        match secondary {
            "updated_at ASC" => "is_pinned DESC, updated_at ASC",
            "created_at DESC" => "is_pinned DESC, created_at DESC",
            "copy_count DESC, updated_at DESC" => "is_pinned DESC, copy_count DESC, updated_at DESC",
            _ => "is_pinned DESC, updated_at DESC",
        }
    }

    /// Build a comma-joined `?,?,…` placeholder list for an `IN (…)` clause.
    pub(super) fn id_placeholders(n: usize) -> String {
        std::iter::repeat_n("?", n).collect::<Vec<_>>().join(",")
    }

    // === Row mapping ===

    /// M-3: Fast path enrichment using cached prefix + string concat.
    /// Relative paths are known-safe (SHA-256 hex filenames from our own code).
    #[inline]
    pub(super) fn enrich_paths(&self, media_path: Option<&str>, thumb_path: Option<&str>) -> (Option<String>, Option<String>) {
        let to_abs = |rel: &str| {
            // Replace '/' with '\\' for Windows; single allocation via format!.
            let normalized = rel.replace('/', "\\");
            format!("{}{}", self.media_root_prefix, normalized)
        };
        let media_abs = media_path.map(to_abs);
        let thumb_abs = thumb_path.map(to_abs);
        (media_abs, thumb_abs)
    }

    pub(super) fn map_record_row(&self, row: &Row<'_>) -> SqlResult<ClipboardRecord> {
        let media_path: Option<String> = row.get(14)?;
        let thumb_path: Option<String> = row.get(15)?;
        // M-3: Pass &str — no clone needed since enrich_paths only reads.
        let (media_abs, thumb_abs) = self.enrich_paths(media_path.as_deref(), thumb_path.as_deref());
        Ok(ClipboardRecord {
            id: row.get(0)?,
            content: row.get(1)?,
            content_type: row.get(2)?,
            source_app: row.get(3)?,
            source_window: row.get(4)?,
            hash: row.get(5)?,
            copy_count: row.get(6)?,
            is_favorite: row.get(7)?,
            is_pinned: row.get(8)?,
            is_sensitive: row.get(9)?,
            is_trashed: row.get(10)?,
            auto_expire_at: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            tags: Vec::new(),
            media_path,
            thumb_path,
            width: row.get(16)?,
            height: row.get(17)?,
            content_html: row.get(18)?,
            media_abs,
            thumb_abs,
            content_len: row.get(19).ok(),
            alias: row.get::<_, String>(20).unwrap_or_default(),
        })
    }

    // === Media helpers ===

    pub(super) fn purge_media_pairs(&self, pairs: &[(Option<String>, Option<String>)]) {
        if pairs.is_empty() {
            return;
        }
        // Dedup only matches ACTIVE rows, so a trashed + an active record can
        // reference the same media/{hash}.png. Reference-count: delete a file
        // only when no remaining row (any state) points at it, otherwise the
        // surviving record's preview/paste would break.
        let mut files: Vec<String> = Vec::new();
        for p in pairs
            .iter()
            .flat_map(|(media_path, thumb_path)| [media_path.as_deref(), thumb_path.as_deref()])
            .flatten()
        {
            if !p.is_empty() {
                files.push(p.to_string());
            }
        }
        files.sort();
        files.dedup();
        if files.is_empty() {
            return;
        }

        let conn = self.lock_read();
        let unreferenced: Vec<String> = files
            .into_iter()
            .filter(|p| {
                conn.query_row(
                    "SELECT 1 FROM records WHERE media_path = ?1 OR thumb_path = ?1 LIMIT 1",
                    [p.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                    == 0
            })
            .collect();
        drop(conn);

        for rel in unreferenced {
            media::delete_media_files(&self.media_root, Some(&rel), None);
        }
    }

    pub(super) fn fetch_media_paths_by_ids(
        &self,
        conn: &Connection,
        ids: &[i64],
    ) -> SqlResult<Vec<(Option<String>, Option<String>)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql)?;
        let pairs = stmt
            .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(pairs)
    }

    /// Batch-load tags for multiple record IDs in one query.
    pub(super) fn load_tags_batch(&self, conn: &Connection, record_ids: &[i64]) -> SqlResult<std::collections::HashMap<i64, Vec<String>>> {
        if record_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = Self::id_placeholders(record_ids.len());
        let sql = format!(
            "SELECT rt.record_id, t.name FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id IN ({})
             ORDER BY rt.record_id",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            record_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut map: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
        for id in record_ids {
            map.entry(*id).or_default();
        }
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (rid, tag_name) = row?;
            if let Some(tags) = map.get_mut(&rid) {
                tags.push(tag_name);
            }
        }
        Ok(map)
    }

    pub(super) fn get_record_tags_locked(&self, conn: &Connection, record_id: i64) -> SqlResult<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT t.name FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id = ?",
        )?;
        let tags = stmt
            .query_map([record_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    }

    // === Record queries ===

    pub fn get_records(
        &self,
        limit: i32,
        offset: i32,
        trashed: bool,
        content_type: Option<&str>,
        favorites_only: bool,
        tag_name: Option<&str>,
        sort: Option<&str>,
        // Keyset cursor (preferred over OFFSET when list mutates via prepend).
        before_pinned: Option<i32>,
        before_updated_at: Option<&str>,
        before_id: Option<i64>,
        include_tags: bool,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut sql = format!(
            "SELECT {} FROM records WHERE is_trashed = ?",
            RECORD_COLS_LIST
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(if trashed { 1i32 } else { 0i32 })];

        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND content_type = ?");
            params.push(Box::new(ct.to_string()));
        }
        if favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }
        if include_tags {
            if let Some(tag) = tag_name.filter(|s| !s.is_empty()) {
                sql.push_str(
                    " AND id IN (
                        SELECT rt.record_id FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE t.name = ?
                    )",
                );
                params.push(Box::new(tag.to_string()));
            }
        }

        // Keyset for default newest-first (+ pinned). Avoids OFFSET drift when
        // clipboard-changed prepends rows while the user scrolls.
        let use_keyset = before_id.is_some()
            && before_updated_at.is_some()
            && matches!(sort.unwrap_or("updated_desc"), "updated_desc");

        if use_keyset {
            let pin = before_pinned.unwrap_or(0);
            let ts = before_updated_at.unwrap().to_string();
            let id = before_id.unwrap();
            // ORDER BY is_pinned DESC, updated_at DESC, id DESC → next page
            sql.push_str(
                " AND (
                    is_pinned < ?
                    OR (is_pinned = ? AND updated_at < ?)
                    OR (is_pinned = ? AND updated_at = ? AND id < ?)
                )",
            );
            params.push(Box::new(pin));
            params.push(Box::new(pin));
            params.push(Box::new(ts.clone()));
            params.push(Box::new(pin));
            params.push(Box::new(ts));
            params.push(Box::new(id));
            sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC, id DESC LIMIT ?");
            params.push(Box::new(limit.max(1)));
        } else {
            sql.push_str(" ORDER BY ");
            sql.push_str(Self::order_by_clause(trashed, sort));
            sql.push_str(" LIMIT ? OFFSET ?");
            params.push(Box::new(limit.max(1)));
            params.push(Box::new(offset.max(0)));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(param_refs.as_slice(), |row| self.map_record_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        if include_tags {
            let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
            let tags_map = self.load_tags_batch(&conn, &ids)?;
            for record in &mut records {
                if let Some(tags) = tags_map.get(&record.id) {
                    record.tags = tags.clone();
                }
            }
        }

        Ok(records)
    }

    pub fn get_record(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE id = ?",
            RECORD_COLS
        ))?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut record = self.map_record_row(row)?;
            record.tags = self.get_record_tags_locked(&conn, record.id)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// List-shaped row (truncated content, no HTML) — cheaper emit after capture.
    pub fn get_record_list(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.lock_read();
        self.get_record_list_locked(&conn, id)
    }

    pub(super) fn get_record_list_locked(
        &self,
        conn: &Connection,
        id: i64,
    ) -> SqlResult<Option<ClipboardRecord>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM records WHERE id = ?",
            RECORD_COLS_LIST
        ))?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut record = self.map_record_row(row)?;
            record.tags = self.get_record_tags_locked(conn, record.id)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Tag names for a record (read lock). Used after auto-tag without reloading the row.
    pub fn get_record_tag_names(&self, record_id: i64) -> SqlResult<Vec<String>> {
        let conn = self.lock_read();
        self.get_record_tags_locked(&conn, record_id)
    }

    /// Full record + bump copy_count in one write lock (paste hot path).
    pub fn take_record_for_paste(&self, id: i64) -> SqlResult<Option<ClipboardRecord>> {
        let conn = self.conn.lock();
        let mut record = {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM records WHERE id = ? AND is_trashed = 0",
                RECORD_COLS
            ))?;
            let mut rows = stmt.query([id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            self.map_record_row(row)?
        };
        record.tags = self.get_record_tags_locked(&conn, record.id)?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE records SET copy_count = copy_count + 1, updated_at = ? WHERE id = ?",
            params![now, id],
        )?;
        record.copy_count = record.copy_count.saturating_add(1);
        Ok(Some(record))
    }

    // === Insert ===

    pub fn insert_record(
        &self,
        content: &str,
        content_type: &ContentType,
        hash: &str,
        is_sensitive: bool,
        max_records: i32,
        sensitive_auto_expire_seconds: i32,
        source_app: &str,
        source_window: &str,
        image: Option<&ImageMeta>,
        content_html: Option<&str>,
    ) -> SqlResult<(i64, bool, ClipboardRecord)> {
        let conn = self.conn.lock();

        // Hash check + insert/update under the same write lock (no TOCTOU between
        // workers; single writer Mutex serializes capture + UI mutations).
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM records WHERE hash = ? AND is_trashed = 0
                 ORDER BY updated_at DESC LIMIT 1",
                [hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            let now = chrono::Utc::now().to_rfc3339();
            // Re-copy only refreshes source/timestamp — paste count is separate.
            conn.execute(
                "UPDATE records SET updated_at = ?, source_app = ?, source_window = ? WHERE id = ?",
                params![now, source_app, source_window, id],
            )?;
            let record = self
                .get_record_list_locked(&conn, id)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
            return Ok((id, false, record));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let auto_expire_at = if is_sensitive && sensitive_auto_expire_seconds > 0 {
            Some((chrono::Utc::now() + chrono::Duration::seconds(sensitive_auto_expire_seconds as i64)).to_rfc3339())
        } else {
            None
        };

        let (media_path, thumb_path, width, height) = match image {
            Some(img) => (
                Some(img.media_path.as_str()),
                Some(img.thumb_path.as_str()),
                Some(img.width),
                Some(img.height),
            ),
            None => (None, None, None, None),
        };

        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, hash, copy_count, is_sensitive, auto_expire_at, created_at, updated_at, media_path, thumb_path, width, height, content_html, content_len)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                content,
                content_type.as_str(),
                source_app,
                source_window,
                hash,
                is_sensitive as i32,
                auto_expire_at,
                now,
                now,
                media_path,
                thumb_path,
                width,
                height,
                content_html,
                content.chars().count() as i64,
            ],
        )?;

        let id = conn.last_insert_rowid();

        // Cheap over-cap probe (scan ≤ max+1 rows). Only then pay for a full COUNT.
        let max = max_records.max(1) as i64;
        let over_cap: bool = conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM records WHERE is_trashed = 0 LIMIT ?
             )",
            [max + 1],
            |row| row.get::<_, i64>(0),
        )? > max;
        if over_cap {
            let active_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM records WHERE is_trashed = 0",
                [],
                |row| row.get(0),
            )?;
            let overflow_count = (active_count - max).max(0);
            // Collect media of records about to be evicted by max_records
            let overflow_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                     ORDER BY updated_at ASC LIMIT ?",
                )?;
                let ids = stmt
                    .query_map([overflow_count], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                ids
            };
            let overflow_media = self.fetch_media_paths_by_ids(&conn, &overflow_ids)?;

            if !overflow_ids.is_empty() {
                let placeholders = Self::id_placeholders(overflow_ids.len());
                let params: Vec<&dyn rusqlite::types::ToSql> =
                    overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                conn.execute(
                    &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                    params.as_slice(),
                )?;
            }
            let record = self
                .get_record_list_locked(&conn, id)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
            drop(conn);
            self.purge_media_pairs(&overflow_media);
            return Ok((id, true, record));
        }

        let record = self
            .get_record_list_locked(&conn, id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        Ok((id, true, record))
    }

    // === Search ===

    pub fn search_records(
        &self,
        query: &str,
        limit: i32,
        offset: i32,
        content_type: Option<&str>,
        favorites_only: bool,
        tag_name: Option<&str>,
        sort: Option<&str>,
        include_tags: bool,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.lock_read();
        let mut sql = format!(
            "SELECT {} FROM records WHERE is_trashed = 0 AND (",
            RECORD_COLS_LIST
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // ≥3 chars: FTS5 trigram. Shorter: single-pass instr (no LIKE '%…%').
        if let Some(fts_match) = Self::build_fts_match_expr(query, include_tags) {
            sql.push_str("id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?)");
            params.push(Box::new(fts_match));
        } else {
            Self::push_short_query_predicate(&mut sql, &mut params, query, include_tags);
        }
        sql.push(')');

        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND content_type = ?");
            params.push(Box::new(ct.to_string()));
        }
        if favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }
        if include_tags {
            if let Some(tag) = tag_name.filter(|s| !s.is_empty()) {
                sql.push_str(
                    " AND id IN (
                        SELECT rt.record_id FROM record_tags rt
                        INNER JOIN tags t ON t.id = rt.tag_id
                        WHERE t.name = ?
                    )",
                );
                params.push(Box::new(tag.to_string()));
            }
        }
        sql.push_str(" ORDER BY ");
        sql.push_str(Self::order_by_clause(false, sort));
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit.max(1)));
        params.push(Box::new(offset.max(0)));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(param_refs.as_slice(), |row| self.map_record_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        if include_tags {
            let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
            let tags_map = self.load_tags_batch(&conn, &ids)?;
            for record in &mut records {
                if let Some(tags) = tags_map.get(&record.id) {
                    record.tags = tags.clone();
                }
            }
        }

        Ok(records)
    }

    // === Delete / Trash ===

    pub fn delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        conn.execute("DELETE FROM records WHERE id = ?", [id])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

    pub fn trash_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE records SET is_trashed = 1, is_pinned = 0 WHERE id = ?",
            [id],
        )?;
        Ok(())
    }

    pub fn trash_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 1, is_pinned = 0 WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        Ok(count)
    }

    pub fn restore_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute("UPDATE records SET is_trashed = 0 WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn restore_records_batch(&self, ids: &[i64]) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "UPDATE records SET is_trashed = 0 WHERE id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        Ok(count)
    }

    pub fn permanently_delete_record(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let media = self.fetch_media_paths_by_ids(&conn, &[id])?;
        let n = conn.execute("DELETE FROM records WHERE id = ? AND is_trashed = 1", [id])?;
        drop(conn);
        if n > 0 {
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
        let placeholders = Self::id_placeholders(ids.len());
        let sql = format!(
            "DELETE FROM records WHERE is_trashed = 1 AND id IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let count = conn.execute(&sql, params.as_slice())?;
        drop(conn);
        if count > 0 {
            self.purge_media_pairs(&media);
        }
        Ok(count)
    }

    pub fn empty_trash(&self) -> SqlResult<usize> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM records WHERE is_trashed = 1")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        let count = conn.execute("DELETE FROM records WHERE is_trashed = 1", [])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(count)
    }

    pub fn get_trash_count(&self) -> SqlResult<i64> {
        let conn = self.lock_read();
        conn.query_row("SELECT COUNT(*) FROM records WHERE is_trashed = 1", [], |row| row.get(0))
    }

    // === Favorites / Pin / Alias ===

    pub fn toggle_favorite(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let current: i32 = conn.query_row(
            "SELECT is_favorite FROM records WHERE id = ?",
            [id],
            |row| row.get(0),
        )?;
        let new_val = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE records SET is_favorite = ? WHERE id = ?",
            params![new_val, id],
        )?;
        Ok(new_val == 1)
    }

    pub fn batch_set_favorite(&self, ids: &[i64], favorite: bool) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(if favorite { 1i32 } else { 0i32 })];
        for id in ids {
            params.push(Box::new(*id));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let n = conn.execute(
            &format!(
                "UPDATE records SET is_favorite = ? WHERE id IN ({placeholders})"
            ),
            param_refs.as_slice(),
        )?;
        Ok(n)
    }

    pub fn toggle_pin(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let current: i32 =
            conn.query_row("SELECT is_pinned FROM records WHERE id = ?", [id], |row| {
                row.get(0)
            })?;
        let new_val = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE records SET is_pinned = ? WHERE id = ?",
            params![new_val, id],
        )?;
        Ok(new_val == 1)
    }

    /// Set short display alias (trim + max 80 chars). Empty clears. Does not touch content/hash.
    pub fn set_record_alias(&self, id: i64, alias: &str) -> SqlResult<String> {
        let mut alias = alias.trim().to_string();
        if alias.chars().count() > ALIAS_MAX_CHARS {
            alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
        }
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE records SET alias = ? WHERE id = ?",
            params![alias, id],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Self::refresh_record_fts(&conn, id)?;
        Ok(alias)
    }

    // === Bulk cleanup ===

    pub fn clear_non_favorite(&self) -> SqlResult<()> {
        let conn = self.conn.lock();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_trashed = 0",
            )?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };
        let media = self.fetch_media_paths_by_ids(&conn, &ids)?;
        conn.execute("DELETE FROM records WHERE is_favorite = 0 AND is_trashed = 0", [])?;
        drop(conn);
        self.purge_media_pairs(&media);
        Ok(())
    }

    // === Import / Export ===

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
            let hashes: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            hashes.into_iter().collect()
        };

        for record in records {
            // M-5: Avoid cloning the entire record (content + content_html can be large).
            // Only create local overrides for fields we might sanitize.
            let mut content_type = crate::security::normalize_content_type(&record.content_type);
            if content_type == "link" && !crate::security::is_openable_link(&record.content) {
                content_type = "text".into();
            }
            let mut media_path = record.media_path.as_deref();
            let mut thumb_path = record.thumb_path.as_deref();
            if let Some(mp) = media_path {
                if !crate::security::is_allowed_media_rel(mp) {
                    media_path = None;
                    thumb_path = None;
                }
            }
            if let Some(tp) = thumb_path {
                if !crate::security::is_allowed_media_rel(tp) {
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
                .filter(|h| crate::security::is_safe_import_html(h));

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
                    content, content_type, source_app, source_window, hash, copy_count,
                    is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                    media_path, thumb_path, width, height, content_html, alias
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.content,
                    content_type,
                    record.source_app,
                    record.source_window,
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

        let active_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 0", [], |row| row.get(0),
        )?;
        let max = max_records.max(1) as i64;
        if active_count > max {
            let overflow_count = active_count - max;
            let overflow_ids: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                     ORDER BY updated_at ASC LIMIT ?",
                )?;
                let ids: Vec<i64> = stmt
                    .query_map([overflow_count], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                ids
            };
            let overflow_media: Vec<(Option<String>, Option<String>)> = {
                if overflow_ids.is_empty() {
                    Vec::new()
                } else {
                    let placeholders = Self::id_placeholders(overflow_ids.len());
                    let sql = format!(
                        "SELECT media_path, thumb_path FROM records WHERE id IN ({})",
                        placeholders
                    );
                    let params: Vec<&dyn rusqlite::types::ToSql> =
                        overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                    let mut stmt = tx.prepare(&sql)?;
                    let pairs: Vec<(Option<String>, Option<String>)> = stmt
                        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
                        .filter_map(|r| r.ok())
                        .collect();
                    pairs
                }
            };

            if !overflow_ids.is_empty() {
                let placeholders = Self::id_placeholders(overflow_ids.len());
                let params: Vec<&dyn rusqlite::types::ToSql> =
                    overflow_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
                tx.execute(
                    &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                    params.as_slice(),
                )?;
            }
            tx.commit()?;
            drop(conn);
            self.purge_media_pairs(&overflow_media);
        } else {
            tx.commit()?;
        }
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
            RECORD_COLS
        ))?;
        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(params![limit, offset], |row| self.map_record_row(row))?
            .collect::<SqlResult<Vec<_>>>()?;
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let tags_map = self.load_tags_batch(&conn, &ids)?;
        for record in &mut records {
            if let Some(tags) = tags_map.get(&record.id) {
                record.tags = tags.clone();
            }
        }
        Ok(records)
    }
}
