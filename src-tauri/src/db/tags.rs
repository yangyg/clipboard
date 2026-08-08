//! Tag CRUD, auto-tag rules, and record↔tag links.
use rusqlite::{params, Connection, Result as SqlResult};

use super::{ClipboardDb, ContentType};
use crate::TagInfo;

/// Fixed 12-color hue wheel (~30° steps). Must stay in sync with
/// `TAG_PALETTE_HEX` in `src/utils/themeColors.ts`.
pub const TAG_PALETTE: &[&str] = &[
    "#ef4444", // red
    "#f97316", // orange
    "#eab308", // amber
    "#84cc16", // lime
    "#22c55e", // green
    "#14b8a6", // teal
    "#06b6d4", // cyan
    "#0ea5e9", // sky
    "#3b82f6", // blue
    "#6366f1", // indigo
    "#a855f7", // purple
    "#ec4899", // pink
];

fn normalize_color_key(color: &str) -> String {
    color.trim().to_ascii_lowercase()
}

fn parse_hex_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let mut h = color.trim().trim_start_matches('#').to_string();
    if h.len() == 3 {
        h = h.chars().flat_map(|c| [c, c]).collect();
    }
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Snap any hex to the nearest palette swatch (RGB Euclidean). Invalid → first swatch.
pub fn nearest_palette_color(color: &str) -> &'static str {
    let key = normalize_color_key(color);
    if let Some(exact) = TAG_PALETTE
        .iter()
        .copied()
        .find(|c| normalize_color_key(c) == key)
    {
        return exact;
    }
    let Some((r, g, b)) = parse_hex_rgb(color) else {
        return TAG_PALETTE[0];
    };
    let mut best = TAG_PALETTE[0];
    let mut best_dist = u32::MAX;
    for swatch in TAG_PALETTE {
        let Some((sr, sg, sb)) = parse_hex_rgb(swatch) else {
            continue;
        };
        let dr = r as i32 - sr as i32;
        let dg = g as i32 - sg as i32;
        let db = b as i32 - sb as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < best_dist {
            best_dist = d;
            best = swatch;
        }
    }
    best
}

/// One-shot: map off-palette tag colors to the nearest swatch.
pub fn migrate_tag_palette_v2(conn: &Connection) -> SqlResult<()> {
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'tag_palette_v2'",
            [],
            |row| row.get(0),
        )
        .ok();
    if done.as_deref() == Some("1") {
        return Ok(());
    }

    let mut stmt = conn.prepare("SELECT id, color FROM tags")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<SqlResult<Vec<_>>>()?;
    drop(stmt);

    for (id, color) in rows {
        let snapped = nearest_palette_color(&color);
        if normalize_color_key(&color) != normalize_color_key(snapped) {
            conn.execute(
                "UPDATE tags SET color = ? WHERE id = ?",
                params![snapped, id],
            )?;
        }
    }

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('tag_palette_v2', '1')",
        [],
    )?;
    Ok(())
}

impl ClipboardDb {
    // === Tag CRUD ===

    /// Tag counts respect the current list facet (type / favorites), exclude trash.
    pub fn get_all_tags(
        &self,
        content_type: Option<&str>,
        favorites_only: bool,
    ) -> SqlResult<Vec<TagInfo>> {
        let conn = self.lock_read();
        let mut sql = String::from(
            "SELECT t.id, t.name, t.color, t.is_auto, COUNT(r.id) as cnt
             FROM tags t
             LEFT JOIN record_tags rt ON rt.tag_id = t.id
             LEFT JOIN records r ON r.id = rt.record_id AND r.is_trashed = 0",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if favorites_only {
            sql.push_str(" AND r.is_favorite = 1");
        }
        if let Some(ct) = content_type.filter(|s| !s.is_empty() && *s != "all") {
            sql.push_str(" AND r.content_type = ?");
            params.push(Box::new(ct.to_string()));
        }

        sql.push_str(" GROUP BY t.id ORDER BY t.name");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let tags = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(TagInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    is_auto: row.get::<_, i32>(3)? != 0,
                    count: row.get(4)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(tags)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> SqlResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?, ?)",
            params![name, color],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn auto_tag_color(name: &str) -> &'static str {
        match name {
            "部署" => "#22c55e",
            "前端" => "#6366f1",
            "链接" => "#eab308",
            "重要" => "#ef4444",
            "设计" => "#a855f7",
            _ => "#6366f1",
        }
    }

    /// Find a tag by name, or create one with `is_auto = 1`.
    pub fn ensure_auto_tag(&self, name: &str) -> SqlResult<i64> {
        let conn = self.conn.lock();
        Self::ensure_auto_tag_conn(&conn, name)
    }

    fn ensure_auto_tag_conn(conn: &Connection, name: &str) -> SqlResult<i64> {
        if let Ok(id) = conn.query_row("SELECT id FROM tags WHERE name = ?", [name], |row| {
            row.get(0)
        }) {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO tags (name, color, is_auto) VALUES (?, ?, 1)",
            params![name, Self::auto_tag_color(name)],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Find a tag by name or create it as non-auto. JSON import and WebDAV pull
    /// use this so tag definitions follow records across devices without
    /// cross-device id collisions (tags merge by name, not by autoincrement id).
    fn ensure_tag_by_name_conn(conn: &Connection, name: &str) -> SqlResult<i64> {
        if let Ok(id) = conn.query_row("SELECT id FROM tags WHERE name = ?", [name], |row| {
            row.get(0)
        }) {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO tags (name, color, is_auto) VALUES (?, ?, 0)",
            params![name, Self::auto_tag_color(name)],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Replace a record's tag links by name inside the caller's transaction.
    /// Dedupes + trims names; missing tags are created via `ensure_tag_by_name_conn`.
    /// Returns `Ok(false)` (leaving record_tags / FTS untouched) when `names` is
    /// empty or identical to the record's current tags — callers rely on that to
    /// avoid wiping local tags from tag-less snapshots and to count real changes.
    /// Rebuilds the record's FTS row once after all links.
    pub(super) fn set_record_tags_by_name_conn(
        conn: &Connection,
        record_id: i64,
        names: &[String],
    ) -> SqlResult<bool> {
        let mut seen: Vec<&str> = Vec::new();
        for name in names {
            let n = name.trim();
            if n.is_empty() || seen.contains(&n) {
                continue;
            }
            seen.push(n);
        }
        if seen.is_empty() {
            return Ok(false);
        }
        let mut seen_sorted = seen.clone();
        seen_sorted.sort_unstable();
        let mut current: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT t.name FROM tags t
                 INNER JOIN record_tags rt ON rt.tag_id = t.id
                 WHERE rt.record_id = ?",
            )?;
            let names = stmt
                .query_map([record_id], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            names
        };
        current.sort_unstable();
        if current == seen_sorted {
            return Ok(false);
        }
        conn.execute("DELETE FROM record_tags WHERE record_id = ?", [record_id])?;
        for name in seen {
            let tag_id = Self::ensure_tag_by_name_conn(conn, name)?;
            conn.execute(
                "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
                params![record_id, tag_id],
            )?;
        }
        Self::refresh_record_fts(conn, record_id)?;
        Ok(true)
    }

    /// Bump `updated_at` so tag-only edits propagate through the record-level
    /// WebDAV merge (last-write-wins by `updated_at`). Caller holds the write lock.
    fn touch_record_updated_at(conn: &Connection, record_id: i64) -> SqlResult<()> {
        conn.execute(
            "UPDATE records SET updated_at = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), record_id],
        )?;
        Ok(())
    }

    /// Apply auto-tag rules to a newly inserted record (OR within each rule).
    /// Single lock + transaction so multiple rules don't re-acquire the DB mutex.
    pub fn apply_auto_tags(
        &self,
        record_id: i64,
        content: &str,
        content_type: &ContentType,
        rules: &[crate::AutoTagRule],
    ) -> SqlResult<()> {
        let ct = content_type.as_str();
        let content_lower = content.to_lowercase();

        let mut matched: Vec<&str> = Vec::new();
        for rule in rules {
            let tag_name = rule.tag_name.trim();
            if tag_name.is_empty() {
                continue;
            }
            let type_hit = rule.content_types.iter().any(|t| t.as_str() == ct);
            let keyword_hit = rule.keywords.iter().any(|kw| {
                let k = kw.trim();
                !k.is_empty() && content_lower.contains(&k.to_lowercase())
            });
            if (type_hit || keyword_hit) && !matched.contains(&tag_name) {
                matched.push(tag_name);
            }
        }
        if matched.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        for tag_name in matched {
            let tag_id = Self::ensure_auto_tag_conn(&tx, tag_name)?;
            tx.execute(
                "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
                params![record_id, tag_id],
            )?;
        }
        // Single FTS rebuild after all tags (no per-INSERT triggers).
        Self::refresh_record_fts(&tx, record_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_tag(&self, id: i64) -> SqlResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let record_ids: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT record_id FROM record_tags WHERE tag_id = ?")?;
            let ids = stmt
                .query_map([id], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        tx.execute("DELETE FROM tags WHERE id = ?", [id])?;
        for record_id in record_ids {
            Self::refresh_record_fts(&tx, record_id)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_tag(&self, id: i64, name: &str, color: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        let old: String =
            conn.query_row("SELECT name FROM tags WHERE id = ?", [id], |row| row.get(0))?;
        conn.execute(
            "UPDATE tags SET name = ?, color = ? WHERE id = ?",
            params![name, color, id],
        )?;
        if old != name {
            // tags_fts_au already rebuilt the affected search rows; bump
            // updated_at so the rename reaches other devices via the
            // record-level sync watermark.
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE records SET updated_at = ? WHERE id IN (
                    SELECT record_id FROM record_tags WHERE tag_id = ?
                 )",
                params![now, id],
            )?;
        }
        Ok(())
    }

    pub fn add_tag_to_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        // Link + watermark + FTS refresh in one transaction so a crash cannot
        // leave a record without its FTS row.
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute(
            "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
            params![record_id, tag_id],
        )?;
        if n > 0 {
            Self::touch_record_updated_at(&tx, record_id)?;
        }
        Self::refresh_record_fts(&tx, record_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_tag_from_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute(
            "DELETE FROM record_tags WHERE record_id = ? AND tag_id = ?",
            params![record_id, tag_id],
        )?;
        if n > 0 {
            Self::touch_record_updated_at(&tx, record_id)?;
        }
        Self::refresh_record_fts(&tx, record_id)?;
        tx.commit()?;
        Ok(())
    }

    /// Replace a record's tags in one transaction (avoids N round-trips from the UI).
    pub fn set_record_tags(&self, record_id: i64, tag_ids: &[i64]) -> SqlResult<()> {
        let conn = self.conn.lock();
        let mut new = tag_ids.to_vec();
        new.sort_unstable();
        new.dedup();
        let mut old: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT tag_id FROM record_tags WHERE record_id = ?")?;
            let ids = stmt
                .query_map([record_id], |row| row.get::<_, i64>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        old.sort_unstable();
        old.dedup();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM record_tags WHERE record_id = ?", [record_id])?;
        for tag_id in &new {
            tx.execute(
                "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
                params![record_id, tag_id],
            )?;
        }
        Self::refresh_record_fts(&tx, record_id)?;
        tx.commit()?;
        if old != new {
            Self::touch_record_updated_at(&conn, record_id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{nearest_palette_color, normalize_color_key, TAG_PALETTE};

    #[test]
    fn palette_has_12_unique_swatches() {
        assert_eq!(TAG_PALETTE.len(), 12);
        let mut keys: Vec<_> = TAG_PALETTE.iter().map(|c| normalize_color_key(c)).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 12);
    }

    #[test]
    fn nearest_returns_exact_match() {
        assert_eq!(nearest_palette_color("#3B82F6"), "#3b82f6");
        assert_eq!(
            normalize_color_key(nearest_palette_color("  #22C55E  ")),
            "#22c55e"
        );
    }

    #[test]
    fn nearest_snaps_off_palette_onto_wheel() {
        let palette: Vec<_> = TAG_PALETTE.iter().map(|c| normalize_color_key(c)).collect();
        for legacy in ["#0078d4", "#60cdff", "#34d399", "#fbbf24", "#a78bfa"] {
            let snapped = normalize_color_key(nearest_palette_color(legacy));
            assert!(
                palette.contains(&snapped),
                "{legacy} snapped to {snapped} which is off-palette"
            );
        }
    }

    #[test]
    fn nearest_invalid_falls_back() {
        assert_eq!(nearest_palette_color("not-a-color"), TAG_PALETTE[0]);
        assert_eq!(nearest_palette_color(""), TAG_PALETTE[0]);
    }

    #[test]
    fn tag_mutations_bump_record_updated_at() {
        use crate::ClipboardDb;
        use crate::ClipboardRecord;

        let dir = std::env::temp_dir().join(format!(
            "clipvault_tag_bump_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let rec = ClipboardRecord {
            id: 0,
            content: "abc".into(),
            content_type: "text".into(),
            source_app: String::new(),
            source_window: String::new(),
            source_name: String::new(),
            hash: "hash-bump".into(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: now.clone(),
            updated_at: now,
            tags: vec![],
            content_html: None,
            media_path: None,
            thumb_path: None,
            width: None,
            height: None,
            media_abs: None,
            thumb_abs: None,
            content_len: None,
            alias: String::new(),
        };
        db.import_records_with_merge(&[rec], 100).unwrap();
        let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;
        let read_updated = || {
            db.get_records_for_export(10, 0)
                .unwrap()
                .remove(0)
                .updated_at
        };
        let before = read_updated();

        std::thread::sleep(std::time::Duration::from_millis(5));
        let tag_id = db.create_tag("bump-tag-1", "#ef4444").unwrap();
        db.add_tag_to_record(record_id, tag_id).unwrap();
        assert_ne!(read_updated(), before);

        std::thread::sleep(std::time::Duration::from_millis(5));
        db.remove_tag_from_record(record_id, tag_id).unwrap();
        assert_ne!(read_updated(), before);

        // Re-applying the same set must NOT bump (no spurious list reorder).
        std::thread::sleep(std::time::Duration::from_millis(5));
        let tag2 = db.create_tag("bump-tag-2", "#eab308").unwrap();
        db.set_record_tags(record_id, &[tag_id, tag2]).unwrap();
        let after_set = read_updated();
        db.set_record_tags(record_id, &[tag_id, tag2]).unwrap();
        assert_eq!(read_updated(), after_set);

        // Renaming a tag must bump every linked record.
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.update_tag(tag_id, "VIP", "#ef4444").unwrap();
        assert_ne!(read_updated(), after_set);

        for name in ["test.db", "test.db-wal", "test.db-shm"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_tag_removes_it_from_full_text_search() {
        use crate::ClipboardDb;
        use crate::ClipboardRecord;

        let dir = std::env::temp_dir().join(format!(
            "clipvault_tag_fts_delete_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let record = ClipboardRecord {
            id: 0,
            content: "searchable content".into(),
            content_type: "text".into(),
            source_app: String::new(),
            source_window: String::new(),
            source_name: String::new(),
            hash: "hash-fts-delete".into(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: now.clone(),
            updated_at: now,
            tags: vec![],
            content_html: None,
            media_path: None,
            thumb_path: None,
            width: None,
            height: None,
            media_abs: None,
            thumb_abs: None,
            content_len: None,
            alias: String::new(),
        };
        db.import_records_with_merge(&[record], 100).unwrap();
        let record_id = db.get_records_for_export(10, 0).unwrap()[0].id;
        let tag_id = db.create_tag("stale-search-tag", "#ef4444").unwrap();
        db.add_tag_to_record(record_id, tag_id).unwrap();

        assert_eq!(
            db.search_records(
                "stale-search-tag",
                10,
                0,
                None,
                false,
                None,
                None,
                true,
                None,
                None,
                None
            )
            .unwrap()
            .len(),
            1
        );
        db.delete_tag(tag_id).unwrap();
        assert!(db
            .search_records(
                "stale-search-tag",
                10,
                0,
                None,
                false,
                None,
                None,
                true,
                None,
                None,
                None
            )
            .unwrap()
            .is_empty());

        for name in ["test.db", "test.db-wal", "test.db-shm"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
