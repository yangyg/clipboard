use crate::media;
use crate::Settings;
use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, Result as SqlResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod records_export;
mod records_flags;
mod records_import;
mod records_media;
mod records_query;
mod records_search;
mod records_trash;
mod records_write;
mod schema;

// Tests for the insert/dedup path live in their own file (mirroring
// schema_tests.rs) to keep records_write.rs under the 500-line cap.
#[cfg(test)]
mod records_write_tests;
// Same split-out pattern keeps records_import.rs / tags.rs under the cap.
#[cfg(test)]
mod records_import_tests;
mod search_history;
mod settings;
mod stats;
mod sync_history;
mod tags;
#[cfg(test)]
mod tags_tests;
mod tombstones;
mod types;

pub use records_export::ExportCursor;
pub use records_import::{validate_import_records, ImportSanitize, MAX_IMPORT_TOTAL_BYTES};
pub use tags::nearest_palette_color;

// Schema compatibility tests live in `schema_tests.rs` (test-only module) to
// keep schema.rs under the 500-line cap.
#[cfg(test)]
mod schema_tests;

pub use types::{
    clamp_page_limit, ContentType, ImageMeta, ALIAS_MAX_CHARS, MAX_PAGE_SIZE, RECORD_COLS,
    RECORD_COLS_LIST,
};

/// Precomputed lowercase auto-tag rules: `(tag_name, lower_keywords, content_types)`.
/// Built once per `Arc<Settings>` identity so the capture hot path never
/// re-lowercases keywords on every clipboard event.
pub(super) struct AutoTagMatcher {
    pub rules: Vec<(String, Vec<String>, Vec<String>)>,
}

impl AutoTagMatcher {
    pub(super) fn from_settings(s: &Settings) -> Self {
        Self {
            rules: s
                .auto_tag_rules
                .iter()
                .map(|r| {
                    (
                        r.tag_name.trim().to_string(),
                        r.keywords.iter().map(|k| k.trim().to_lowercase()).collect(),
                        r.content_types
                            .iter()
                            .map(|t| t.as_str().to_string())
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

/// Precomputed lowercase ignore-app patterns: `(full pattern lower, basename
/// lower without .exe)`. Same shape as the panel matcher, cached per settings
/// identity so captures skip repeated lowercase allocations.
pub(super) struct IgnoredAppPatterns {
    pub patterns: Vec<(String, String)>,
}

impl IgnoredAppPatterns {
    pub(super) fn from_settings(s: &Settings) -> Self {
        Self {
            patterns: s
                .ignored_apps
                .iter()
                .map(|p| {
                    let lower = p.trim().to_lowercase();
                    let noext = lower.strip_suffix(".exe").unwrap_or(&lower).to_string();
                    (lower, noext)
                })
                .collect(),
        }
    }
}

pub struct ClipboardDb {
    /// Writer connection (schema, inserts, updates, deletes).
    conn: Mutex<Connection>,
    /// Small pool of query_only readers — WAL allows concurrent reads with the writer
    /// and with each other (unlike a single Mutex around one read conn).
    read_conns: Vec<Mutex<Connection>>,
    read_rr: std::sync::atomic::AtomicUsize,
    media_root: PathBuf,
    /// M-3: Cached string prefix (with trailing separator) for fast path enrichment.
    /// Avoids PathBuf alloc + to_string_lossy per record row.
    media_root_prefix: String,
    /// In-memory copy of `app_settings`, populated lazily. Avoids re-parsing the
    /// settings JSON on every clipboard event (the monitor reads it 2-3x/event).
    /// Arc allows cheap clone on the capture hot path (atomic refcount bump only).
    settings_cache: RwLock<Option<Arc<Settings>>>,
    /// Cached lowercase matchers, keyed by `Arc<Settings>` identity. Invalidation
    /// is free: `save_settings` swaps in a fresh Arc, so `Arc::ptr_eq` is a
    /// cheap generation check.
    auto_tag_cache: RwLock<Option<(Arc<Settings>, Arc<AutoTagMatcher>)>>,
    ignored_cache: RwLock<Option<(Arc<Settings>, Arc<IgnoredAppPatterns>)>>,
    /// get_stats TTL cache — the stats page polls during copy bursts, and the
    /// underlying aggregate is a full-table scan, so serve a 5s snapshot.
    stats_cache: Mutex<Option<(std::time::Instant, crate::StatsData)>>,
    /// Ciphertext cache for DPAPI-encrypted secrets so repeated settings saves
    /// (debounced slider drags) do not re-encrypt unchanged secrets.
    secrets_cache: Mutex<Option<(String, String, String, String)>>,
}

const READ_POOL_SIZE: usize = 3;

impl ClipboardDb {
    fn configure_connection(conn: &Connection, query_only: bool) -> SqlResult<()> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             PRAGMA cache_size=-16384;
             PRAGMA temp_store=MEMORY;",
        )?;
        if query_only {
            // Fail loudly if a "read" path accidentally tries to mutate.
            conn.execute_batch("PRAGMA query_only=ON;")?;
        }
        Ok(())
    }

    pub fn new(db_path: &Path, media_root: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        Self::configure_connection(&conn, false)?;

        Self::initialize_schema(&conn)?;

        // FTS must exist before any migration touches records:
        // migrate_text_hash_v2 deletes/merges rows and calls refresh_record_fts,
        // both of which require records_fts + its triggers. On a legacy DB that
        // predates FTS (or where FTS creation previously failed), running the
        // migrations first would fail and block app startup entirely.
        Self::ensure_fts(&conn)?;

        // One-time backfill of content_len (avoids length(content) on every list query).
        let backfilled: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'content_len_backfill'",
                [],
                |row| row.get(0),
            )
            .ok();
        if backfilled.as_deref() != Some("1") {
            conn.execute(
                "UPDATE records SET content_len = length(content) WHERE content_len = 0",
                [],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('content_len_backfill', '1')",
                [],
            )?;
        }

        // Snap legacy tag hex values onto the fixed 12-color hue wheel.
        tags::migrate_tag_palette_v2(&conn)?;

        // Re-derive text hashes from plain content and merge the duplicates
        // the old html-in-hash scheme produced (capture now hashes text only).
        Self::migrate_text_hash_v2(&conn)?;

        // --- Schema version gate ---
        // All migrations above are idempotent (CREATE IF NOT EXISTS / ALTER … .ok()).
        // After they run, stamp the expected version so doctor can verify it.
        Self::apply_schema_version(&conn)?;

        // media/ dirs were already created by the caller (lib.rs::run) before
        // the DB opens; a failure here is recorded rather than swallowed so a
        // missing media root is visible in logs instead of failing later at
        // image-capture time with no trace.
        if let Err(e) = media::ensure_dirs(&media_root) {
            tracing::warn!("Failed to create media directories: {}", e);
        }

        // Reader pool: open after schema is ready (same DB file, WAL).
        let mut read_conns = Vec::with_capacity(READ_POOL_SIZE);
        for _ in 0..READ_POOL_SIZE {
            let c = Connection::open(db_path)?;
            Self::configure_connection(&c, true)?;
            read_conns.push(Mutex::new(c));
        }

        // M-3: Pre-compute string prefix for enrich_paths (avoids per-record PathBuf).
        let media_root_prefix = {
            let mut s = media_root.to_string_lossy().to_string();
            if !s.ends_with('\\') && !s.ends_with('/') {
                s.push('\\');
            }
            s
        };

        Ok(Self {
            conn: Mutex::new(conn),
            read_conns,
            read_rr: std::sync::atomic::AtomicUsize::new(0),
            media_root,
            media_root_prefix,
            settings_cache: RwLock::new(None),
            auto_tag_cache: RwLock::new(None),
            ignored_cache: RwLock::new(None),
            stats_cache: Mutex::new(None),
            secrets_cache: Mutex::new(None),
        })
    }

    #[inline]
    pub(super) fn lock_read(&self) -> parking_lot::MutexGuard<'_, Connection> {
        let n = self.read_conns.len();
        let start = self
            .read_rr
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % n;
        for o in 0..n {
            if let Some(g) = self.read_conns[(start + o) % n].try_lock() {
                return g;
            }
        }
        // L-5: All readers busy — log contention for diagnostics before blocking.
        tracing::debug!("DB read pool exhausted ({} conns); blocking on lock", n);
        self.read_conns[start].lock()
    }

    /// Return the lowercase auto-tag matcher for `settings`, rebuilding only
    /// when the settings snapshot identity changes.
    pub(super) fn auto_tag_matcher(&self, settings: &Arc<Settings>) -> Arc<AutoTagMatcher> {
        {
            let cache = self.auto_tag_cache.read();
            if let Some((cached, matcher)) = cache.as_ref() {
                if Arc::ptr_eq(cached, settings) {
                    return matcher.clone();
                }
            }
        }
        let matcher = Arc::new(AutoTagMatcher::from_settings(settings));
        *self.auto_tag_cache.write() = Some((Arc::clone(settings), matcher.clone()));
        matcher
    }

    /// Ignore-app check with cached lowercase patterns (hot capture path).
    /// Semantics mirror `crate::panel::is_ignored_app` (basename with optional
    /// `.exe` suffix, or the full path, case-insensitive; no substring match).
    pub fn is_ignored_app(&self, source_app: &str, settings: &Arc<Settings>) -> bool {
        if source_app.is_empty() {
            return false;
        }
        let patterns = {
            let cache = self.ignored_cache.read();
            match cache.as_ref() {
                Some((cached, patterns)) if Arc::ptr_eq(cached, settings) => patterns.clone(),
                _ => {
                    drop(cache);
                    let patterns = Arc::new(IgnoredAppPatterns::from_settings(settings));
                    *self.ignored_cache.write() = Some((Arc::clone(settings), patterns.clone()));
                    patterns
                }
            }
        };
        if patterns.patterns.is_empty() {
            return false;
        }
        let app_lower = source_app.to_lowercase();
        let basename = source_app
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(source_app)
            .to_lowercase();
        let basename_noext = basename.strip_suffix(".exe").unwrap_or(&basename);
        patterns
            .patterns
            .iter()
            .any(|(full, noext)| basename_noext == noext || app_lower == *full)
    }

    pub fn media_root(&self) -> &Path {
        &self.media_root
    }
}
