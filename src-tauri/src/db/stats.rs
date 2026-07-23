//! Aggregate stats for the settings / overview page.
use rusqlite::Result as SqlResult;

use super::ClipboardDb;
use crate::media;
use crate::StatsData;

impl ClipboardDb {
    // === Stats ===

    pub fn get_stats(&self) -> SqlResult<StatsData> {
        let conn = self.lock_read();

        // One table scan: aggregates + per-type counts (known content_type values).
        let row: (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(copy_count), 0),
                    SUM(CASE WHEN is_favorite = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN is_pinned = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN is_sensitive = 1 THEN 1 ELSE 0 END),
                    COALESCE(SUM(content_len), 0)
                      + COALESCE(SUM(length(COALESCE(content_html, ''))), 0),
                    SUM(CASE WHEN content_type = 'text' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN content_type = 'code' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN content_type = 'link' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN content_type = 'image' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN content_type = 'file' THEN 1 ELSE 0 END)
             FROM records WHERE is_trashed = 0",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )?;

        let (
            total_records,
            total_copies,
            favorites_count,
            pinned_count,
            sensitive_count,
            content_bytes,
            n_text,
            n_code,
            n_link,
            n_image,
            n_file,
        ) = row;

        let mut type_distribution = std::collections::HashMap::new();
        type_distribution.insert("text".into(), n_text);
        type_distribution.insert("code".into(), n_code);
        type_distribution.insert("link".into(), n_link);
        type_distribution.insert("image".into(), n_image);
        type_distribution.insert("file".into(), n_file);
        drop(conn);

        let media_bytes = media::cached_media_dir_size(&self.media_root);
        let storage_bytes = content_bytes.saturating_add(media_bytes);

        Ok(StatsData {
            total_records,
            total_copies,
            favorites_count,
            pinned_count,
            sensitive_count,
            storage_bytes,
            data_path: self.media_root.display().to_string(),
            type_distribution,
        })
    }
}
