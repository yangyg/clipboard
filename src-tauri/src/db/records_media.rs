//! Media-path bookkeeping: purging unreferenced files, batch tag loading.
use rusqlite::{Connection, Result as SqlResult};

use super::ClipboardDb;
use crate::media;

impl ClipboardDb {
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
            .collect::<SqlResult<Vec<_>>>()?;
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
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(tags)
    }
}
