//! Full-text / short-query search over records.
use rusqlite::Result as SqlResult;

use super::{ClipboardDb, RECORD_COLS_LIST};
use crate::ClipboardRecord;

impl ClipboardDb {
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
        Self::push_tag_filter(&mut sql, &mut params, tag_name, include_tags);
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
            .collect::<SqlResult<Vec<_>>>()?;

        self.enrich_tags(&conn, &mut records, include_tags)?;

        Ok(records)
    }
}
