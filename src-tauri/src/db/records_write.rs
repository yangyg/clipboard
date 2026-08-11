//! Record inserts, dedup, capacity eviction, and the hash-v2 migration.
//! Trash/restore live in `records_trash.rs`; favorite/pin/alias in `records_flags.rs`.
use rusqlite::{params, Connection, Result as SqlResult};

use super::{ClipboardDb, ContentType, ImageMeta};
use crate::detect::sha256_hash;
use crate::ClipboardRecord;

/// Legacy text row snapshot for the hash-v2 migration:
/// (id, is_favorite, is_pinned, copy_count, alias, is_trashed, updated_at)
type LegacyHashRow = (i64, i32, i32, i32, String, i32, String);

impl ClipboardDb {
    // === Insert ===

    pub fn insert_record(
        &self,
        content: &str,
        content_type: &ContentType,
        hash: &str,
        is_sensitive: bool,
        max_records: i32,
        sensitive_auto_expire_seconds: i32,
        source_app: &str,
        source_window: &str,
        source_name: &str,
        image: Option<&ImageMeta>,
        content_html: Option<&str>,
    ) -> SqlResult<(i64, bool, ClipboardRecord)> {
        // Stamp the originating device on every fresh capture. The identity is
        // generated at startup; an empty value (settings load failure / tests)
        // degrades to an unknown-origin row rather than failing the insert.
        let source_device_id = self
            .get_settings()
            .map(|s| s.webdav_device_id.clone())
            .unwrap_or_default();
        let conn = self.conn.lock();
        if let Some(id) = Self::find_active_duplicate(&conn, hash)? {
            let record =
                self.refresh_duplicate_source(&conn, id, source_app, source_window, source_name)?;
            return Ok((id, false, record));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let auto_expire_at = Self::sensitive_expiry(is_sensitive, sensitive_auto_expire_seconds);
        let id = Self::insert_new_row(
            &conn,
            content,
            content_type,
            hash,
            is_sensitive,
            auto_expire_at.clone(),
            source_app,
            source_window,
            source_name,
            &source_device_id,
            image,
            content_html,
            &now,
        )?;
        // Build the returned list-shape record in memory — every field is
        // already known here, so the fresh-insert path skips the row read-back
        // (2 extra queries per capture under the write lock). Tags are loaded
        // separately by the auto-tag flow when enabled.
        let record = self.build_inserted_record(
            id,
            content,
            content_type,
            hash,
            is_sensitive,
            auto_expire_at,
            source_app,
            source_window,
            source_name,
            &source_device_id,
            image,
            &now,
        );
        if !Self::is_over_capacity(&conn, max_records)? {
            return Ok((id, true, record));
        }
        let overflow_media = self.evict_over_limit(&conn, max_records)?;
        drop(conn);
        self.purge_media_pairs(&overflow_media);
        Ok((id, true, record))
    }

    /// List-shape `ClipboardRecord` for a just-inserted row, built from the
    /// values the insert already holds (no DB round-trip). `content` is
    /// truncated to 400 chars to match `RECORD_COLS_LIST`; `content_len` keeps
    /// the full character count.
    fn build_inserted_record(
        &self,
        id: i64,
        content: &str,
        content_type: &ContentType,
        hash: &str,
        is_sensitive: bool,
        auto_expire_at: Option<String>,
        source_app: &str,
        source_window: &str,
        source_name: &str,
        source_device_id: &str,
        image: Option<&ImageMeta>,
        now: &str,
    ) -> ClipboardRecord {
        let (media_path, thumb_path, width, height) = match image {
            Some(img) => (
                Some(img.media_path.as_str()),
                Some(img.thumb_path.as_str()),
                Some(img.width),
                Some(img.height),
            ),
            None => (None, None, None, None),
        };
        let (media_abs, thumb_abs) = self.enrich_paths(media_path, thumb_path);
        const LIST_CONTENT_MAX: usize = 400;
        let list_content: String = content.chars().take(LIST_CONTENT_MAX).collect();
        ClipboardRecord {
            id,
            content: list_content,
            content_type: content_type.as_str().to_string(),
            source_app: source_app.to_string(),
            source_window: source_window.to_string(),
            source_name: source_name.to_string(),
            source_device_id: source_device_id.to_string(),
            hash: hash.to_string(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive,
            is_trashed: false,
            auto_expire_at,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            tags: Vec::new(),
            content_html: None,
            media_path: media_path.map(str::to_string),
            thumb_path: thumb_path.map(str::to_string),
            width,
            height,
            media_abs,
            thumb_abs,
            content_len: Some(content.chars().count() as i32),
            alias: String::new(),
        }
    }

    /// Hash probe for the dedup path. Must run under the same write lock as
    /// the insert (no TOCTOU between capture workers). A real read error must
    /// not be mistaken for "no match" — that would insert a duplicate row
    /// instead of deduping. Trashed rows are excluded to mirror the partial
    /// unique index `uq_records_hash_active`: re-copying a trashed item
    /// inserts a fresh record instead of reviving the trash row.
    fn find_active_duplicate(conn: &Connection, hash: &str) -> SqlResult<Option<i64>> {
        match conn.query_row(
            "SELECT id FROM records WHERE hash = ? AND is_trashed = 0
             ORDER BY updated_at DESC LIMIT 1",
            [hash],
            |row| row.get(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Re-copy of an existing record: refresh source/timestamp only (paste
    /// count is a separate action). FTS indexes source_app/source_window, but
    /// the content-only trigger never fires here — refresh the FTS row only
    /// when the source actually changed, so searching by the new source still
    /// matches.
    fn refresh_duplicate_source(
        &self,
        conn: &Connection,
        id: i64,
        source_app: &str,
        source_window: &str,
        source_name: &str,
    ) -> SqlResult<ClipboardRecord> {
        let source_changed: bool = conn.query_row(
            "SELECT source_app != ?1 OR source_window != ?2 OR source_name != ?3
             FROM records WHERE id = ?4",
            params![source_app, source_window, source_name, id],
            |row| row.get(0),
        )?;
        conn.execute(
            "UPDATE records SET updated_at = ?, source_app = ?, source_window = ?, source_name = ? WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), source_app, source_window, source_name, id],
        )?;
        if source_changed {
            Self::refresh_record_fts(conn, id)?;
        }
        let (_, _, record) = self.read_back_inserted(conn, id)?;
        Ok(record)
    }

    /// Expiry timestamp for sensitive captures; `None` when the feature is off.
    fn sensitive_expiry(is_sensitive: bool, auto_expire_seconds: i32) -> Option<String> {
        if !is_sensitive || auto_expire_seconds <= 0 {
            return None;
        }
        Some(
            (chrono::Utc::now() + chrono::Duration::seconds(auto_expire_seconds as i64))
                .to_rfc3339(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_new_row(
        conn: &Connection,
        content: &str,
        content_type: &ContentType,
        hash: &str,
        is_sensitive: bool,
        auto_expire_at: Option<String>,
        source_app: &str,
        source_window: &str,
        source_name: &str,
        source_device_id: &str,
        image: Option<&ImageMeta>,
        content_html: Option<&str>,
        now: &str,
    ) -> SqlResult<i64> {
        let (media_path, thumb_path, width, height) = match image {
            Some(img) => (
                Some(img.media_path.as_str()),
                Some(img.thumb_path.as_str()),
                Some(img.width),
                Some(img.height),
            ),
            None => (None, None, None, None),
        };
        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, source_name, source_device_id, hash, copy_count, is_sensitive, auto_expire_at, created_at, updated_at, media_path, thumb_path, width, height, content_html, content_len)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                content,
                content_type.as_str(),
                source_app,
                source_window,
                source_name,
                source_device_id,
                hash,
                is_sensitive as i32,
                auto_expire_at,
                now,
                now,
                media_path,
                thumb_path,
                width,
                height,
                content_html,
                content.chars().count() as i64,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Cheap over-cap probe (scan ≤ max+1 rows); the caller only pays for a
    /// full eviction when this says yes.
    fn is_over_capacity(conn: &Connection, max_records: i32) -> SqlResult<bool> {
        let max = max_records.max(1) as i64;
        let probe: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM records WHERE is_trashed = 0 LIMIT ?
             )",
            [max + 1],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(probe > max)
    }

    /// Read back the just-written row as the list-shape payload.
    fn read_back_inserted(
        &self,
        conn: &Connection,
        id: i64,
    ) -> SqlResult<(i64, bool, ClipboardRecord)> {
        let record = self
            .get_record_list_locked(conn, id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        Ok((id, true, record))
    }

    /// Cheap existence probe used by startup history import to skip records
    /// that already exist (active **or** trashed). Deliberately distinct from
    /// `insert_record`'s dedup-update path: importing an existing item must NOT
    /// bump `updated_at` or reset `source_*` to empty (re-ranking the list every
    /// session). Any-row matching mirrors the `UNIQUE(hash)` index — a hash that
    /// only exists in the trash would otherwise slip the probe and make the
    /// subsequent `insert_record` fail with a UNIQUE constraint violation.
    pub fn record_hash_exists(&self, hash: &str) -> SqlResult<bool> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM records WHERE hash = ? LIMIT 1",
            [hash],
            |_| Ok(()),
        )
        .map(|_| true)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })
    }

    /// Evict oldest non-favorite / non-pinned active rows when `max_records` is
    /// exceeded. Returns the media pairs of evicted rows; callers must release
    /// the write lock before passing them to `purge_media_pairs` (which takes a
    /// read lock). Shared by insert + import so capacity rules stay in sync.
    pub(super) fn evict_over_limit(
        &self,
        conn: &Connection,
        max_records: i32,
    ) -> SqlResult<Vec<(Option<String>, Option<String>)>> {
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM records WHERE is_trashed = 0",
            [],
            |row| row.get(0),
        )?;
        let max = max_records.max(1) as i64;
        if active_count <= max {
            return Ok(Vec::new());
        }
        let overflow_count = active_count - max;
        let overflow_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM records WHERE is_favorite = 0 AND is_pinned = 0 AND is_trashed = 0
                 ORDER BY updated_at ASC LIMIT ?",
            )?;
            let ids = stmt
                .query_map([overflow_count], |row| row.get(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        let overflow_media = self.fetch_media_paths_by_ids(conn, &overflow_ids)?;
        if !overflow_ids.is_empty() {
            let placeholders = Self::id_placeholders(overflow_ids.len());
            let params: Vec<&dyn rusqlite::types::ToSql> = overflow_ids
                .iter()
                .map(|id| id as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(
                &format!("DELETE FROM records WHERE id IN ({placeholders})"),
                params.as_slice(),
            )?;
        }
        Ok(overflow_media)
    }

    /// One-shot (settings flag `text_hash_v2`): re-derive text-record hashes
    /// from plain content and merge the duplicates the old scheme created.
    ///
    /// Historical hashes baked CF_HTML bytes into the fingerprint, so the same
    /// text copied from a different source (or re-written by our own paste)
    /// hashed differently and inserted a duplicate row. New identity is
    /// sha256(sha256(text)) — matching what capture stores now. Rows that
    /// collide after re-derivation are merged into the most recently updated
    /// one: favorite/pin OR'd, copy_count summed, tags unioned.
    pub(super) fn migrate_text_hash_v2(conn: &Connection) -> SqlResult<()> {
        let done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'text_hash_v2'",
                [],
                |row| row.get(0),
            )
            .ok();
        if done.as_deref() == Some("1") {
            return Ok(());
        }

        // Group candidate rows by the re-derived hash BEFORE writing anything:
        // updating hashes row-by-row would trip the unique-hash constraint
        // mid-way, because two legacy rows with identical content re-derive to
        // the same hash and the second UPDATE collides before any merge runs.
        let groups = Self::group_text_rows_by_rederived_hash(conn)?;
        for (new_hash, group) in groups {
            Self::apply_rederived_hash_group(conn, &new_hash, group)?;
        }

        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('text_hash_v2', '1')",
            [],
        )?;
        Ok(())
    }

    /// Scan text rows and bucket them by sha256(sha256(content)). Image rows
    /// hash pixels, not content — they are skipped.
    fn group_text_rows_by_rederived_hash(
        conn: &Connection,
    ) -> SqlResult<std::collections::HashMap<String, Vec<LegacyHashRow>>> {
        let mut groups: std::collections::HashMap<String, Vec<LegacyHashRow>> =
            std::collections::HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT id, content, is_favorite, is_pinned, copy_count, alias, is_trashed, updated_at
             FROM records
             WHERE content_type != 'image' AND media_path IS NULL",
        )?;
        let mapped = stmt.query_map([], |row| {
            let content: String = row.get(1)?;
            Ok((
                sha256_hash(&sha256_hash(&content)),
                (
                    row.get(0)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ),
            ))
        })?;
        for item in mapped {
            let (hash, entry) = item?;
            groups.entry(hash).or_default().push(entry);
        }
        Ok(groups)
    }

    /// Merge one re-derived-hash group into a single survivor and stamp the
    /// new hash last — with losers already deleted the unique-hash constraint
    /// can never fire.
    fn apply_rederived_hash_group(
        conn: &Connection,
        new_hash: &str,
        mut group: Vec<LegacyHashRow>,
    ) -> SqlResult<()> {
        if group.len() == 1 {
            conn.execute(
                "UPDATE records SET hash = ? WHERE id = ?",
                params![new_hash, group[0].0],
            )?;
            return Ok(());
        }
        let (winner_id, fav, pin, count, alias, loser_ids) =
            Self::fold_group_into_winner(&mut group);
        conn.execute(
            "UPDATE records SET is_favorite = ?, is_pinned = ?, copy_count = ?, alias = ?
             WHERE id = ?",
            params![fav as i32, pin as i32, count, alias, winner_id],
        )?;
        for loser in &loser_ids {
            conn.execute(
                "INSERT OR IGNORE INTO record_tags (record_id, tag_id)
                 SELECT ?, tag_id FROM record_tags WHERE record_id = ?",
                params![winner_id, loser],
            )?;
        }
        Self::refresh_record_fts(conn, winner_id)?;
        // FTS row + record_tags links of losers cascade on delete.
        let placeholders = Self::id_placeholders(loser_ids.len());
        let params: Vec<&dyn rusqlite::types::ToSql> = loser_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        conn.execute(
            &format!("DELETE FROM records WHERE id IN ({placeholders})"),
            params.as_slice(),
        )?;
        conn.execute(
            "UPDATE records SET hash = ? WHERE id = ?",
            params![new_hash, winner_id],
        )?;
        Ok(())
    }

    /// Pick the survivor (active first, then most recently updated) and fold
    /// loser state into it: favorite/pin OR'd, copy_count summed, alias
    /// back-filled. Trashed losers contribute nothing — they just vanish.
    fn fold_group_into_winner(
        group: &mut Vec<LegacyHashRow>,
    ) -> (i64, bool, bool, i32, String, Vec<i64>) {
        group.sort_by(|a, b| {
            a.5.cmp(&b.5) // is_trashed ASC (active first)
                .then(b.6.cmp(&a.6)) // updated_at DESC
                .then(b.0.cmp(&a.0)) // id DESC
        });
        let (winner_id, fav, pin, mut count, mut alias, _, _) = group.remove(0);
        let mut fav = fav != 0;
        let mut pin = pin != 0;
        let mut loser_ids: Vec<i64> = Vec::new();
        for (id, f, p, c, a, trashed, _) in group.iter() {
            loser_ids.push(*id);
            if *trashed == 0 {
                fav |= *f != 0;
                pin |= *p != 0;
                count += c;
                if alias.is_empty() && !a.is_empty() {
                    alias = a.clone();
                }
            }
        }
        (winner_id, fav, pin, count, alias, loser_ids)
    }

    // === Delete / Trash / Favorites / Pin / Alias ===
    // Moved to `records_trash.rs` and `records_flags.rs` to keep each file
    // under the size cap; all remain `impl ClipboardDb` methods.
}
