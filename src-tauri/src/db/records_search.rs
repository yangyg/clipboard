//! Full-text / short-query search over records.
use rusqlite::Result as SqlResult;

use super::{ClipboardDb, PageCursor, RECORD_COLS_LIST};
use crate::ClipboardRecord;

/// Upper bound on FTS candidates materialized before the outer sort/keyset
/// pass. A pathological query matching tens of thousands of rows should not
/// force a full sort of every hit; beyond this the page is already deep enough
/// that truncation is imperceptible (list UI soft-caps at ~120 rows, and page
/// 2+ uses keyset cursors over the same truncated candidate set). Measured at
/// 50k rows: 10k candidates cost ~230ms p50 to sort; 2k stays comfortably
/// under the 50ms p95 search target (docs/perf.md).
const FTS_CANDIDATE_LIMIT: i64 = 2_000;

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
        cursor: PageCursor<'_>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.lock_read();
        let mut sql: String;
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // ≥3 chars: FTS5 trigram. Shorter: single-pass instr (no LIKE '%…%').
        if let Some(fts_match) = Self::build_fts_match_expr(query, include_tags) {
            // Drive the outer query from the bounded FTS candidate set and
            // probe records by primary key. The previous `id IN (subquery)`
            // shape made the planner scan every active row and test
            // membership — measured ~80ms p50 at 50k rows even with the LIMIT.
            // CROSS JOIN pins the join order (subquery first) so SQLite cannot
            // flip back to the 50k-row covering-index scan.
            sql = format!(
                "SELECT {} FROM records
                 CROSS JOIN (SELECT rowid FROM records_fts
                             WHERE records_fts MATCH ?
                             ORDER BY rank LIMIT {}) f ON f.rowid = records.id
                 WHERE is_trashed = 0",
                RECORD_COLS_LIST, FTS_CANDIDATE_LIMIT
            );
            params.push(Box::new(fts_match));
        } else {
            sql = format!(
                "SELECT {} FROM records WHERE is_trashed = 0 AND (",
                RECORD_COLS_LIST
            );
            let single_char = query.chars().count() == 1;
            Self::push_short_query_predicate(
                &mut sql,
                &mut params,
                query,
                include_tags,
                !single_char,
            );
            sql.push(')');
        }

        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND content_type = ?");
            params.push(Box::new(ct.to_string()));
        }
        if favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }
        Self::push_tag_filter(&mut sql, &mut params, tag_name, include_tags);

        // Keyset for every whitelist sort, mirroring get_records. Avoids OFFSET
        // drift when a new matching clip is captured while the user scrolls.
        ClipboardDb::push_pagination_tail(
            &mut sql,
            &mut params,
            cursor.is_ready(sort),
            cursor,
            limit,
            offset,
            false,
            sort,
        );

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
