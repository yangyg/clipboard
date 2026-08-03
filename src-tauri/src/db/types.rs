//! Shared DB types and column-list constants (extracted from `db/mod.rs` to
//! keep the connection-pool module small).
use std::fmt;

/// Content categories a captured clipboard payload can be classified into.
/// `is_sensitive` is a separate boolean flag, not a ContentType variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Code,
    Link,
    Image,
    File,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentType::Text => write!(f, "text"),
            ContentType::Code => write!(f, "code"),
            ContentType::Link => write!(f, "link"),
            ContentType::Image => write!(f, "image"),
            ContentType::File => write!(f, "file"),
        }
    }
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Code => "code",
            ContentType::Link => "link",
            ContentType::Image => "image",
            ContentType::File => "file",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "text" => ContentType::Text,
            "code" => ContentType::Code,
            "link" => ContentType::Link,
            "image" => ContentType::Image,
            "file" => ContentType::File,
            _ => ContentType::Text,
        }
    }
}

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
               content_len, alias";

/// List/search: omit HTML, truncate content for IPC/memory; prefer content_len column.
pub const RECORD_COLS_LIST: &str = "id, substr(content, 1, 400) as content, content_type, source_app, source_window, hash,
               copy_count, is_favorite, is_pinned, is_sensitive, is_trashed, auto_expire_at,
               created_at, updated_at, media_path, thumb_path, width, height, NULL as content_html,
               content_len, alias";

pub const ALIAS_MAX_CHARS: usize = 80;
