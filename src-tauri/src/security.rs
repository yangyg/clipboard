//! Path / import hardening helpers shared by IPC commands and DB import.

use std::path::{Component, Path, PathBuf};

use regex::Regex;
use std::sync::LazyLock;

static MEDIA_REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^media/(?:thumbs/)?[a-f0-9]{64}\.(?:png|jpe?g)$").unwrap()
});

const ALLOWED_CONTENT_TYPES: &[&str] = &["text", "code", "link", "image", "file"];

/// Relative media keys we accept in DB / import (hash-named files only).
pub fn is_allowed_media_rel(rel: &str) -> bool {
    let norm = rel.trim().replace('\\', "/");
    if norm.is_empty() || norm.contains('\0') {
        return false;
    }
    MEDIA_REL_RE.is_match(&norm)
}

pub fn normalize_content_type(raw: &str) -> String {
    let t = raw.trim().to_lowercase();
    if ALLOWED_CONTENT_TYPES.contains(&t.as_str()) {
        t
    } else {
        "text".into()
    }
}

/// http(s) only — blocks javascript:/data:/file: etc.
pub fn is_safe_http_url(s: &str) -> bool {
    let trimmed = s.trim();
    let Ok(url) = url::Url::parse(trimmed) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
}

/// Export/import JSON path: absolute, `.json`, no `..`, parent exists (export) / file exists (import).
pub fn validate_json_io_path(path: &str, for_write: bool) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("无效的文件路径".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("路径必须是绝对路径".into());
    }
    for c in p.components() {
        if matches!(c, Component::ParentDir) {
            return Err("路径不能包含 '..'".into());
        }
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("json");
    if !ext {
        return Err("仅允许 .json 文件".into());
    }
    if for_write {
        match p.parent() {
            Some(parent) if parent.as_os_str().is_empty() => {}
            Some(parent) if parent.exists() => {}
            Some(_) => return Err("导出目录不存在".into()),
            None => return Err("无效的导出路径".into()),
        }
    } else {
        if !p.is_file() {
            return Err("导入文件不存在".into());
        }
    }
    Ok(p)
}

/// Resolve a relative media path under app data; canonicalize + prefix check.
pub fn resolve_media_file(app_data_dir: &Path, relative: &str) -> Result<PathBuf, String> {
    if !is_allowed_media_rel(relative) {
        return Err("非法媒体路径".into());
    }
    let abs = crate::media::absolute(app_data_dir, relative);
    let root = app_data_dir
        .canonicalize()
        .unwrap_or_else(|_| app_data_dir.to_path_buf());
    let canon = abs
        .canonicalize()
        .map_err(|_| format!("图片文件不存在: {}", abs.display()))?;
    if !canon.starts_with(&root) {
        return Err("路径不在媒体目录内".into());
    }
    if !canon.is_file() {
        return Err(format!("图片文件不存在: {}", canon.display()));
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_rel_accepts_hash_png() {
        let h = "a".repeat(64);
        assert!(is_allowed_media_rel(&format!("media/{h}.png")));
        assert!(is_allowed_media_rel(&format!("media/thumbs/{h}.jpg")));
    }

    #[test]
    fn media_rel_rejects_traversal_and_cmd_meta() {
        assert!(!is_allowed_media_rel("media/../secrets.png"));
        assert!(!is_allowed_media_rel("media/foo&calc.exe.png"));
        assert!(!is_allowed_media_rel("media/short.png"));
        assert!(!is_allowed_media_rel("C:\\Windows\\System32\\a.png"));
    }

    #[test]
    fn http_url_filter() {
        assert!(is_safe_http_url("https://example.com/a"));
        assert!(is_safe_http_url("http://example.com"));
        assert!(!is_safe_http_url("javascript:alert(1)"));
        assert!(!is_safe_http_url("data:text/html,hi"));
        assert!(!is_safe_http_url("file:///c:/x"));
    }

    #[test]
    fn json_path_requires_json_ext() {
        assert!(validate_json_io_path("C:\\tmp\\out.txt", true).is_err());
    }
}
