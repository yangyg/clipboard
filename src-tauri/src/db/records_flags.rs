//! Record flag mutations: favorite, pin, alias.
use rusqlite::{params, Result as SqlResult};

use super::{ClipboardDb, ALIAS_MAX_CHARS};

/// SQLite stores booleans as integers in `records` flag columns.
const FLAG_OFF: i32 = 0;
const FLAG_ON: i32 = 1;

impl ClipboardDb {
    pub fn toggle_favorite(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        // Single atomic flip (no read-then-write race), returning the new value.
        let new_val: i32 = conn.query_row(
            "UPDATE records SET is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END
             WHERE id = ? RETURNING is_favorite",
            [id],
            |row| row.get(0),
        )?;
        Ok(new_val == FLAG_ON)
    }

    pub fn batch_set_favorite(&self, ids: &[i64], favorite: bool) -> SqlResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let placeholders = Self::id_placeholders(ids.len());
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(if favorite { FLAG_ON } else { FLAG_OFF })];
        for id in ids {
            params.push(Box::new(*id));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let n = conn.execute(
            &format!("UPDATE records SET is_favorite = ? WHERE id IN ({placeholders})"),
            param_refs.as_slice(),
        )?;
        Ok(n)
    }

    pub fn toggle_pin(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock();
        let new_val: i32 = conn.query_row(
            "UPDATE records SET is_pinned = CASE WHEN is_pinned = 1 THEN 0 ELSE 1 END
             WHERE id = ? RETURNING is_pinned",
            [id],
            |row| row.get(0),
        )?;
        Ok(new_val == FLAG_ON)
    }

    /// Set short display alias (trim + max 80 chars). Empty clears. Does not touch content/hash.
    pub fn set_record_alias(&self, id: i64, alias: &str) -> SqlResult<String> {
        let mut alias = alias.trim().to_string();
        if alias.chars().count() > ALIAS_MAX_CHARS {
            alias = alias.chars().take(ALIAS_MAX_CHARS).collect();
        }
        let conn = self.conn.lock();
        // UPDATE + FTS refresh in one transaction: a crash between the two
        // would otherwise drop the FTS row permanently (search misses).
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE records SET alias = ? WHERE id = ?",
            params![alias, id],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Self::refresh_record_fts(&tx, id)?;
        tx.commit()?;
        Ok(alias)
    }
}
