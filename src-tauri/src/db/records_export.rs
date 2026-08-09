//! Full-content export/backup paging (never uses list truncation columns).
use rusqlite::{params, Connection, Result as SqlResult};

use super::{clamp_page_limit, ClipboardDb, RECORD_COLS};
use crate::ClipboardRecord;

/// Keyset cursor for export paging: (is_pinned, updated_at, id) matches the
/// export ORDER BY so pages never skip or duplicate rows.
#[derive(Debug, Clone)]
pub struct ExportCursor {
    pub is_pinned: bool,
    pub updated_at: String,
    pub id: i64,
}

impl ClipboardDb {
    /// OFFSET-based page — kept for tests / one-shot exports. Prefer
    /// `get_records_for_export_page` for iterating large datasets.
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
            .query_map(params![clamp_page_limit(limit), offset.max(0)], |row| {
                self.map_record_row(row)
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        self.strip_local_paths(&conn, &mut records)?;
        Ok(records)
    }

    pub fn get_records_for_export_page(
        &self,
        limit: i32,
        cursor: Option<&ExportCursor>,
    ) -> SqlResult<Vec<ClipboardRecord>> {
        let conn = self.lock_read();
        let mut sql = format!("SELECT {} FROM records WHERE is_trashed = 0", RECORD_COLS);
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
        values.push(Box::new(clamp_page_limit(limit)));
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|value| value.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut records: Vec<ClipboardRecord> = stmt
            .query_map(refs.as_slice(), |row| self.map_record_row(row))?
            .collect::<SqlResult<Vec<_>>>()?;
        self.strip_local_paths(&conn, &mut records)?;
        Ok(records)
    }

    /// Export bundles travel to other machines / backups: absolute media paths
    /// are local-only and must never leak into the payload. Tags are enriched
    /// because the bundle must be self-contained.
    fn strip_local_paths(
        &self,
        conn: &Connection,
        records: &mut [ClipboardRecord],
    ) -> SqlResult<()> {
        self.enrich_tags(conn, records, true)?;
        for record in records.iter_mut() {
            record.media_abs = None;
            record.thumb_abs = None;
        }
        Ok(())
    }
}
