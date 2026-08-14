//! Standalone tag-definition sync (`tags.json`): snapshot rows, LWW merge, GC.
//! CRUD and auto-tag stay in `tags.rs`. `ClipboardDb` remains the public facade.

use rusqlite::{params, Result as SqlResult};

use super::tags::bump_tag_epoch;
use super::ClipboardDb;

/// One tag definition as carried by the standalone tags sync (tags.json). Tag
/// edits propagate through this snapshot keyed on `updated_at` (LWW) instead of
/// rewriting every linked record's `updated_at`.
#[derive(Debug, Clone)]
pub struct TagSyncRow {
    pub name: String,
    pub color: String,
    pub is_auto: bool,
    pub updated_at: String,
}

/// Result of applying a remote tag snapshot: `tags_pulled` on the wire is
/// `added + changed + deleted`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TagMergeStats {
    pub added: i32,
    pub changed: i32,
    pub deleted: i32,
}

/// `(tag_name, deleted_at)` pairs carried by the tag sync.
pub type TagTombstoneRows = Vec<(String, String)>;

/// Epoch sentinel stamped on legacy rows when `tags.updated_at` ships. It
/// keeps a post-upgrade merge from stamping migration time over real remote
/// edits, and protects never-touched rows from the conservative GC.
pub const TAG_EPOCH_SENTINEL: &str = "1970-01-01T00:00:00Z";

impl ClipboardDb {
    /// Full tag definitions + deletion tombstones, for publishing tags.json.
    pub fn get_tag_sync_rows(&self) -> SqlResult<(Vec<TagSyncRow>, TagTombstoneRows)> {
        let conn = self.lock_read();
        let mut stmt = conn.prepare("SELECT name, color, is_auto, updated_at FROM tags")?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagSyncRow {
                    name: row.get(0)?,
                    color: row.get(1)?,
                    is_auto: row.get::<_, i32>(2)? != 0,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        let mut stmt = conn.prepare("SELECT name, deleted_at FROM tag_tombstones")?;
        let tombstones = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok((tags, tombstones))
    }

    /// Merge a remote tags.json snapshot into the local tag definitions, LWW by
    /// `updated_at`:
    /// - newer incoming rows update color/is_auto and advance the stamp;
    /// - tags absent locally and not tombstoned (or tombstoned older) are added;
    /// - incoming tombstones delete local tags not edited after the deletion
    ///   (affected records get one batched FTS rebuild);
    /// - conservative GC: local tags absent from the snapshot, never touched
    ///   since the snapshot, and linked to zero records are removed (no
    ///   tombstone — this is not a real delete, a device that still has the tag
    ///   can re-add it). Never-touched sentinel rows (fresh installs / re-seeds)
    ///   are exempt so built-in defaults survive a first pull.
    pub fn merge_tag_snapshot(
        &self,
        incoming: &[TagSyncRow],
        incoming_tombstones: &[(String, String)],
        remote_snapshot_updated_at: &str,
    ) -> SqlResult<TagMergeStats> {
        let mut conn = self.lock_write();
        let tx = conn.transaction()?;
        let mut stats = TagMergeStats::default();

        // 1. Fold incoming tombstones into the local table (keep the newest per name).
        for (name, deleted_at) in incoming_tombstones {
            tx.execute(
                "INSERT INTO tag_tombstones (name, deleted_at) VALUES (?, ?)
                 ON CONFLICT(name) DO UPDATE SET deleted_at = MAX(deleted_at, excluded.deleted_at)",
                params![name, deleted_at],
            )?;
        }

        // 2. Apply incoming tag definitions (LWW by updated_at).
        let mut local: std::collections::HashMap<String, (i64, String, bool, String)> = {
            let mut stmt = tx.prepare("SELECT id, name, color, is_auto, updated_at FROM tags")?;
            let rows: Vec<(String, (i64, String, bool, String))> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        (
                            row.get(0)?,
                            row.get(2)?,
                            row.get::<_, i32>(3)? != 0,
                            row.get(4)?,
                        ),
                    ))
                })?
                .collect::<SqlResult<_>>()?;
            rows.into_iter().collect()
        };
        for tag in incoming {
            if let Some((id, lcolor, lauto, lupdated)) = local.get(&tag.name).cloned() {
                if tag.updated_at.as_str() <= lupdated.as_str() {
                    continue; // local is at least as new — nothing to do
                }
                let color_changed = tag.color != lcolor;
                let auto_changed = tag.is_auto != lauto;
                if color_changed || auto_changed {
                    tx.execute(
                        "UPDATE tags SET color = ?, is_auto = ? WHERE id = ?",
                        params![tag.color, tag.is_auto as i32, id],
                    )?;
                    stats.changed += 1;
                }
                // Always advance the stamp so a re-merge of the same snapshot is
                // a no-op even when color/is_auto are identical.
                tx.execute(
                    "UPDATE tags SET updated_at = ? WHERE id = ?",
                    params![tag.updated_at, id],
                )?;
                if let Some(entry) = local.get_mut(&tag.name) {
                    entry.1 = tag.color.clone();
                    entry.2 = tag.is_auto;
                    entry.3 = tag.updated_at.clone();
                }
            } else {
                // Tombstone gate: deleted elsewhere at/after this stamp → don't resurrect.
                let tombstone: Option<String> = tx
                    .query_row(
                        "SELECT deleted_at FROM tag_tombstones WHERE name = ?",
                        [&tag.name],
                        |row| row.get(0),
                    )
                    .ok();
                if let Some(deleted_at) = tombstone {
                    if deleted_at.as_str() >= tag.updated_at.as_str() {
                        continue;
                    }
                }
                tx.execute(
                    "INSERT INTO tags (name, color, is_auto, updated_at) VALUES (?, ?, ?, ?)",
                    params![tag.name, tag.color, tag.is_auto as i32, tag.updated_at],
                )?;
                tx.execute("DELETE FROM tag_tombstones WHERE name = ?", [&tag.name])?;
                let new_id = tx.last_insert_rowid();
                local.insert(
                    tag.name.clone(),
                    (
                        new_id,
                        tag.color.clone(),
                        tag.is_auto,
                        tag.updated_at.clone(),
                    ),
                );
                stats.added += 1;
            }
        }

        // 3. Incoming tombstone deletes (a local tag edited after the deletion
        //    wins and is kept; its tombstone still blocks stale bundles).
        let mut fts_dirty: Vec<i64> = Vec::new();
        for (name, deleted_at) in incoming_tombstones {
            let Some((id, lupdated)) = local.get(name).map(|(id, _, _, u)| (*id, u.clone())) else {
                continue;
            };
            if lupdated.as_str() > deleted_at.as_str() {
                continue; // local edit wins
            }
            let mut stmt = tx.prepare("SELECT record_id FROM record_tags WHERE tag_id = ?")?;
            let ids = stmt
                .query_map([id], |row| row.get(0))?
                .collect::<SqlResult<Vec<i64>>>()?;
            fts_dirty.extend(ids);
            tx.execute("DELETE FROM tags WHERE id = ?", [id])?;
            local.remove(name);
            stats.deleted += 1;
        }

        // 4. Conservative GC: local tags the snapshot doesn't know, last touched
        //    before the snapshot, with zero linked records — safe local cleanup.
        let incoming_names: std::collections::HashSet<&str> =
            incoming.iter().map(|t| t.name.as_str()).collect();
        let stale: Vec<String> = local
            .iter()
            .filter(|(name, (_, _, _, updated))| {
                !incoming_names.contains(name.as_str())
                    && updated.as_str() != TAG_EPOCH_SENTINEL
                    && updated.as_str() < remote_snapshot_updated_at
            })
            .map(|(name, _)| name.clone())
            .collect();
        for name in stale {
            let (id, _, _, _) = local[&name];
            let links: i64 = tx.query_row(
                "SELECT COUNT(*) FROM record_tags WHERE tag_id = ?",
                [id],
                |row| row.get(0),
            )?;
            if links == 0 {
                tx.execute("DELETE FROM tags WHERE id = ?", [id])?;
                local.remove(&name);
                stats.deleted += 1;
            }
        }

        if !fts_dirty.is_empty() {
            fts_dirty.sort_unstable();
            fts_dirty.dedup();
            Self::refresh_records_fts_batch(&tx, &fts_dirty)?;
        }
        tx.commit()?;
        bump_tag_epoch();
        Ok(stats)
    }
}
