//! Path / import hardening helpers shared by IPC commands and DB import.

use std::path::{Component, Path, PathBuf};

use regex::Regex;
use std::sync::LazyLock;

static MEDIA_REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^media/(?:thumbs/)?[a-f0-9]{64}\.(?:png|jpe?g)$").unwrap()
});

// Event-handler attributes that could execute when the HTML is pasted into a
// rich-text editor. Scoped to known handler names so plain text like "one="
// never triggers a false positive.
static HTML_HANDLER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bon(?:click|dblclick|mousedown|mouseup|mouseover|mouseout|mousemove|mouseenter|mouseleave|load|error|unload|focus|blur|change|submit|keydown|keypress|keyup|drag|dragstart|dragend|dragover|drop|input|scroll|wheel|pointerdown|pointerup|pointerover|pointerout|touchstart|touchend|contextmenu|select|toggle|animationstart|transitionend|auxclick)\s*=",
    )
    .unwrap()
});

const ALLOWED_CONTENT_TYPES: &[&str] = &["text", "code", "link", "image", "file"];

/// Conservative guard for `content_html` arriving from untrusted boundaries
/// (JSON import / WebDAV pull). Local capture is always the user's own clipboard
/// and skips this. Returns false → the DB layer drops HTML (keeps plain text),
/// so the blob can never be re-pasted into a rich-text editor. False positives
/// only cost formatting, never content.
pub fn is_safe_import_html(html: &str) -> bool {
    if html.is_empty() {
        return true;
    }
    let lower = html.to_lowercase();
    const BLOCKED_TAGS: &[&str] = &[
        "<script",
        "<style",
        "<iframe",
        "<object",
        "<embed",
        "<form",
        "<base",
        "<link",
        "<meta",
        "<svg",
        "<math",
        "<template",
    ];
    if BLOCKED_TAGS.iter().any(|t| lower.contains(t)) {
        return false;
    }
    if HTML_HANDLER_RE.is_match(&lower) {
        return false;
    }
    if lower.contains("javascript:")
        || lower.contains("vbscript:")
        || lower.contains("file:")
    {
        return false;
    }
    true
}

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

/// Schemes accepted as `content_type: link` and allowed by `open_url`.
const LINK_SCHEMES: &[&str] = &["http", "https", "ftp", "magnet", "ed2k", "thunder"];

/// Case-insensitive whole-string prefixes when `Url::parse` is awkward (e.g. ed2k pipes).
const LINK_PREFIXES: &[&str] = &[
    "https://",
    "http://",
    "ftp://",
    "magnet:",
    "ed2k://",
    "thunder://",
];

/// True when the trimmed string is a whole openable link URI (http(s)/ftp/magnet/ed2k/thunder).
pub fn is_openable_link(s: &str) -> bool {
    link_scheme(s).is_some()
}

fn scheme_has_body(trimmed: &str, scheme: &str) -> bool {
    // Reject bare `magnet:` / `http://` with nothing after the conventional prefix.
    let lower = trimmed.to_ascii_lowercase();
    for &prefix in LINK_PREFIXES {
        if prefix.starts_with(scheme) && lower.starts_with(prefix) {
            return trimmed.len() > prefix.len();
        }
    }
    // Unknown prefix shape for a known scheme — require at least `scheme:` + 1 char.
    trimmed.len() > scheme.len() + 1
}

/// Returns the lowercase scheme if `s` is entirely one whitelisted link URI.
pub fn link_scheme(s: &str) -> Option<&'static str> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return None;
    }
    if let Ok(url) = url::Url::parse(trimmed) {
        let scheme = url.scheme();
        if let Some(&known) = LINK_SCHEMES.iter().find(|&&k| k == scheme) {
            if scheme_has_body(trimmed, known) {
                return Some(known);
            }
            return None;
        }
        // Parsed but not whitelisted (javascript:, data:, file:, …)
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for &prefix in LINK_PREFIXES {
        if lower.starts_with(prefix) && trimmed.len() > prefix.len() {
            let scheme = prefix.split([':', '/']).next().unwrap_or(prefix);
            if let Some(&known) = LINK_SCHEMES.iter().find(|&&k| k == scheme) {
                return Some(known);
            }
        }
    }
    None
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
    fn media_rel_rejects_drive_letter_segment() {
        // `media::absolute` joins segment-by-segment; on Windows a `C:x` segment
        // replaces the accumulated path and resolves against the process CWD,
        // escaping the media root — the strict hash-path regex must reject it.
        assert!(!is_allowed_media_rel("media/C:evil.txt"));
        assert!(!is_allowed_media_rel("media/thumbs/C:\\evil.jpg"));
        assert!(!is_allowed_media_rel("C:evil.txt"));
        assert!(!is_allowed_media_rel("media/D:\\outside.png"));
    }

    #[test]
    fn openable_link_whitelist() {
        assert_eq!(link_scheme("https://example.com"), Some("https"));
        assert_eq!(link_scheme("https://example.com/a"), Some("https"));
        assert_eq!(link_scheme("  HTTP://Example.COM/a "), Some("http"));
        assert_eq!(link_scheme("ftp://host/file"), Some("ftp"));
        assert_eq!(
            link_scheme("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"),
            Some("magnet")
        );
        assert_eq!(
            link_scheme("ed2k://|file|name.iso|123|ABCDEF0123456789ABCDEF0123456789|/"),
            Some("ed2k")
        );
        assert_eq!(link_scheme("thunder://QUFodHRwOi8vZXhhbXBsZS5jb20v"), Some("thunder"));
        assert!(!is_openable_link("javascript:alert(1)"));
        assert!(!is_openable_link("data:text/html,hi"));
        assert!(!is_openable_link("file:///c:/x"));
        assert!(!is_openable_link("magnet:"));
        assert!(!is_openable_link("http://"));
        assert!(!is_openable_link("see magnet:?xt=urn:btih:abc more text"));
        assert!(!is_openable_link("plain text"));
        // http(s) subset still openable; download schemes are not "browser-only" but are openable
        assert!(matches!(link_scheme("https://a.com"), Some("https")));
        assert!(matches!(link_scheme("magnet:?xt=urn:btih:abc"), Some("magnet")));
    }

    #[test]
    fn import_html_rejects_executable_markup() {
        assert!(is_safe_import_html("<p>hello <b>world</b></p>"));
        assert!(is_safe_import_html("<p>set one=2 and two=3</p>"));
        assert!(!is_safe_import_html("<p>x<script>alert(1)</script></p>"));
        assert!(!is_safe_import_html("<img src=x onerror=alert(1)>"));
        assert!(!is_safe_import_html("<a href=\"javascript:alert(1)\">x</a>"));
        assert!(!is_safe_import_html("<iframe src=\"https://evil\"></iframe>"));
        assert!(!is_safe_import_html("<svg onload=alert(1)>"));
    }

    #[test]
    fn json_path_requires_json_ext() {
        assert!(validate_json_io_path("C:\\tmp\\out.txt", true).is_err());
    }
}
