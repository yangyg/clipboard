//! Tag CRUD, auto-tag rules, and record↔tag links.
use rusqlite::{params, Connection, Result as SqlResult};

use super::{ClipboardDb, ContentType};
use crate::TagInfo;

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
            .filter_map(|r| r.ok())
            .collect();
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
            "部署" => "#34d399",
            "前端" => "#6366f1",
            "链接" => "#fbbf24",
            _ => "#6366f1",
        }
    }

    /// Find a tag by name, or create one with `is_auto = 1`.
    pub fn ensure_auto_tag(&self, name: &str) -> SqlResult<i64> {
        let conn = self.conn.lock();
        Self::ensure_auto_tag_conn(&conn, name)
    }

    fn ensure_auto_tag_conn(conn: &Connection, name: &str) -> SqlResult<i64> {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM tags WHERE name = ?",
            [name],
            |row| row.get(0),
        ) {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO tags (name, color, is_auto) VALUES (?, ?, 1)",
            params![name, Self::auto_tag_color(name)],
        )?;
        Ok(conn.last_insert_rowid())
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
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tags WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn update_tag(&self, id: i64, name: &str, color: &str) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tags SET name = ?, color = ? WHERE id = ?",
            params![name, color, id],
        )?;
        Ok(())
    }

    pub fn add_tag_to_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
            params![record_id, tag_id],
        )?;
        Self::refresh_record_fts(&conn, record_id)?;
        Ok(())
    }

    pub fn remove_tag_from_record(&self, record_id: i64, tag_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM record_tags WHERE record_id = ? AND tag_id = ?",
            params![record_id, tag_id],
        )?;
        Self::refresh_record_fts(&conn, record_id)?;
        Ok(())
    }

    /// Replace a record's tags in one transaction (avoids N round-trips from the UI).
    pub fn set_record_tags(&self, record_id: i64, tag_ids: &[i64]) -> SqlResult<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM record_tags WHERE record_id = ?", [record_id])?;
        for tag_id in tag_ids {
            tx.execute(
                "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?, ?)",
                params![record_id, tag_id],
            )?;
        }
        Self::refresh_record_fts(&tx, record_id)?;
        tx.commit()?;
        Ok(())
    }
}
