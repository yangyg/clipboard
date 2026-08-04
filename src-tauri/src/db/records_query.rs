//! Record list/detail queries + row-mapping helpers.
//! (Part of the records-module split: query / write / search / import-export / media.)
use rusqlite::{params, Connection, Result as SqlResult, Row};

use super::{ClipboardDb, RECORD_COLS, RECORD_COLS_LIST};
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
            source_name: row.get::<_, String>(21).unwrap_or_default(),
        })
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
            .collect::<SqlResult<Vec<_>>>()?;

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
}

#[cfg(test)]
mod tests {
    use super::ClipboardDb;

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(ClipboardDb::escape_like("100%"), "100\\%");
        assert_eq!(ClipboardDb::escape_like("a_b"), "a\\_b");
        assert_eq!(ClipboardDb::escape_like("c:\\x"), "c:\\\\x");
        assert_eq!(ClipboardDb::escape_like("plain"), "plain");
    }

    #[test]
    fn fts_match_needs_three_chars() {
        assert_eq!(ClipboardDb::build_fts_match("ab"), None);
        assert_eq!(ClipboardDb::build_fts_match("  x "), None);
        assert_eq!(ClipboardDb::build_fts_match("abc"), Some("\"abc\"".to_string()));
    }

    #[test]
    fn fts_match_escapes_quotes() {
        assert_eq!(
            ClipboardDb::build_fts_match(r#"a"b"#),
            Some("\"a\"\"b\"".to_string())
        );
    }

    #[test]
    fn fts_match_expr_excludes_tags_when_disabled() {
        assert_eq!(
            ClipboardDb::build_fts_match_expr("abc", true),
            Some("\"abc\"".to_string())
        );
        assert_eq!(
            ClipboardDb::build_fts_match_expr("abc", false),
            Some("{content alias source_app source_window}: \"abc\"".to_string())
        );
        assert_eq!(ClipboardDb::build_fts_match_expr("ab", false), None);
    }

    #[test]
    fn placeholders_join_count() {
        assert_eq!(ClipboardDb::id_placeholders(0), "");
        assert_eq!(ClipboardDb::id_placeholders(1), "?");
        assert_eq!(ClipboardDb::id_placeholders(3), "?,?,?");
    }
}
