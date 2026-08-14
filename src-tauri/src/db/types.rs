//! Shared DB types and column-list constants (extracted from `db/mod.rs` to
//! keep the connection-pool module small).

pub use crate::types::ContentType;

/// Optional image metadata when inserting an image record.
#[derive(Debug, Clone)]
pub struct ImageMeta {
    pub media_path: String,
    pub thumb_path: String,
    pub width: i32,
    pub height: i32,
}

/// Full row including rich HTML (detail / paste / export).
pub const RECORD_COLS: &str = "id, content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, content_html,
               content_len, alias, source_name, source_device_id";

/// List/search: omit HTML, truncate content for IPC/memory; prefer content_len column.
pub const RECORD_COLS_LIST: &str =
    "id, substr(content, 1, 400) as content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, NULL as content_html,
               content_len, alias, source_name, source_device_id";

pub const ALIAS_MAX_CHARS: usize = 80;

/// Upper bound for caller-supplied page sizes (IPC / sync). Guards against a
/// malformed or hostile limit pulling the entire table in a single query.
pub const MAX_PAGE_SIZE: i32 = 500;

/// Clamp a page size into `[1, MAX_PAGE_SIZE]`.
pub fn clamp_page_limit(limit: i32) -> i32 {
    limit.clamp(1, MAX_PAGE_SIZE)
}
