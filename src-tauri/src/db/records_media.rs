//! Media-path bookkeeping: purging unreferenced files, batch tag loading.
use rusqlite::{Connection, Result as SqlResult};

use super::ClipboardDb;
use crate::media;
use crate::ClipboardRecord;

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
        // Single batched reference probe instead of one query per file (the
        // old path ran N SELECTs twice per bulk delete). A file is referenced
        // when any row points at it as media_path OR thumb_path.
        let referenced = match Self::referenced_media_set(&conn, &files) {
            Ok(set) => set,
            Err(e) => {
                // Conservative: on a probe error, delete nothing. A stray file
                // is safer than breaking a surviving record's preview/paste.
                tracing::warn!("Failed to probe media references; skipping purge: {}", e);
                return;
            }
        };
        drop(conn);
        let unreferenced: Vec<String> = files
            .into_iter()
            .filter(|p| !referenced.contains(p))
            .collect();

        // Quarantine each unreferenced file, then re-check references before
        // finishing the delete. A concurrent capture of the same image content
        // lands on the same hash path; if its row was inserted between the
        // scan above and the quarantine, deleting would break its preview.
        let mut quarantined: Vec<(String, std::path::PathBuf)> = Vec::new();
        for rel in unreferenced {
            if let Some(pending) = media::quarantine_media_file(&self.media_root, &rel) {
                quarantined.push((rel, pending));
            }
        }
        if quarantined.is_empty() {
            return;
        }

        let conn = self.lock_read();
        let mut removed: u64 = 0;
        let quarantined_rels: Vec<String> =
            quarantined.iter().map(|(rel, _)| rel.clone()).collect();
        let still_referenced = match Self::referenced_media_set(&conn, &quarantined_rels) {
            Ok(set) => set,
            Err(e) => {
                // Restore every quarantined file so nothing is lost when the
                // re-check cannot run.
                tracing::warn!(
                    "Failed to re-check media references; restoring quarantine: {}",
                    e
                );
                for (rel, pending) in quarantined {
                    media::restore_media_file(&self.media_root, &rel, &pending);
                }
                drop(conn);
                return;
            }
        };
        for (rel, pending) in quarantined {
            if still_referenced.contains(&rel) {
                media::restore_media_file(&self.media_root, &rel, &pending);
            } else if let Ok(meta) = std::fs::metadata(&pending) {
                let size = meta.len();
                if std::fs::remove_file(&pending).is_err() {
                    continue;
                }
                removed = removed.saturating_add(size);
            }
        }
        drop(conn);
        media::note_media_removed(&self.media_root, removed);
    }

    /// One batched `media_path/thumb_path IN (...)` probe returning the set of
    /// referenced relative paths.
    fn referenced_media_set(
        conn: &Connection,
        files: &[String],
    ) -> SqlResult<std::collections::HashSet<String>> {
        if files.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let placeholders = Self::id_placeholders(files.len());
        let refs: Vec<&dyn rusqlite::types::ToSql> = files
            .iter()
            .map(|f| f as &dyn rusqlite::types::ToSql)
            .collect();
        let bound: Vec<&dyn rusqlite::types::ToSql> =
            refs.iter().copied().chain(refs.iter().copied()).collect();
        let mut stmt = conn.prepare(&format!(
            "SELECT media_path FROM records WHERE media_path IN ({placeholders})
             UNION
             SELECT thumb_path FROM records WHERE thumb_path IN ({placeholders})"
        ))?;
        let rows = stmt.query_map(bound.as_slice(), |row| row.get::<_, String>(0))?;
        let mut set = std::collections::HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
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
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let mut stmt = conn.prepare(&sql)?;
        let pairs = stmt
            .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(pairs)
    }

    /// Batch-load tags for multiple record IDs in one query.
    pub(super) fn load_tags_batch(
        &self,
        conn: &Connection,
        record_ids: &[i64],
    ) -> SqlResult<std::collections::HashMap<i64, Vec<String>>> {
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
        let params: Vec<&dyn rusqlite::types::ToSql> = record_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
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

    pub(super) fn get_record_tags_locked(
        &self,
        conn: &Connection,
        record_id: i64,
    ) -> SqlResult<Vec<String>> {
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

    /// Append `AND id IN (SELECT record_id … WHERE t.name = ?)` when a tag filter
    /// applies. Shared by list + search queries (must stay in sync).
    pub(super) fn push_tag_filter(
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
        tag_name: Option<&str>,
        include_tags: bool,
    ) {
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
    }

    /// Assign tags onto list rows via one batch query (records must be mutable).
    pub(super) fn enrich_tags(
        &self,
        conn: &Connection,
        records: &mut [ClipboardRecord],
        include_tags: bool,
    ) -> SqlResult<()> {
        if !include_tags || records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let mut tags_map = self.load_tags_batch(conn, &ids)?;
        for record in records.iter_mut() {
            if let Some(tags) = tags_map.get_mut(&record.id) {
                record.tags = std::mem::take(tags);
            }
        }
        Ok(())
    }

    /// Batch-load (name, color) tag pairs for multiple record IDs — used by the
    /// export path so tag colors travel with the sync bundle.
    pub(super) fn load_tag_colors_batch(
        &self,
        conn: &Connection,
        record_ids: &[i64],
    ) -> SqlResult<std::collections::HashMap<i64, Vec<(String, String)>>> {
        if record_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = Self::id_placeholders(record_ids.len());
        let sql = format!(
            "SELECT rt.record_id, t.name, t.color FROM tags t
             INNER JOIN record_tags rt ON rt.tag_id = t.id
             WHERE rt.record_id IN ({}) ORDER BY rt.record_id",
            placeholders
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = record_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut map: std::collections::HashMap<i64, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for id in record_ids {
            map.entry(*id).or_default();
        }
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (rid, name, color) = row?;
            if let Some(pairs) = map.get_mut(&rid) {
                pairs.push((name, color));
            }
        }
        Ok(map)
    }

    /// Assign (name, color) pairs onto records via one batch query (export path).
    pub(super) fn enrich_tag_colors(
        &self,
        conn: &Connection,
        records: &mut [ClipboardRecord],
    ) -> SqlResult<()> {
        if records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let mut colors_map = self.load_tag_colors_batch(conn, &ids)?;
        for record in records.iter_mut() {
            if let Some(pairs) = colors_map.get_mut(&record.id) {
                record.tag_colors = std::mem::take(pairs);
            }
        }
        Ok(())
    }
}
