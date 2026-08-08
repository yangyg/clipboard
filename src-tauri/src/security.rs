//! Path / import hardening helpers shared by IPC commands and DB import.

use std::path::{Component, Path, PathBuf};

use regex::Regex;
use std::sync::LazyLock;

static MEDIA_REL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^media/(?:thumbs/)?[a-f0-9]{64}\.(?:png|jpe?g)$").unwrap());

// Event-handler attributes that could execute when the HTML is pasted into a
// rich-text editor. Scoped to known handler names so plain text like "one="
// never triggers a false positive.
//
// Keep the list additive: it must cover every `on*` attribute a hostile HTML
// blob could carry. In particular pointer/touch/drag handlers and media-event
// handlers (`<video oncanplay=...>`) were historically missing.
const HTML_HANDLER_NAMES: &[&str] = &[
    "click",
    "dblclick",
    "auxclick",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseover",
    "mouseout",
    "mouseenter",
    "mouseleave",
    "contextmenu",
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "pointercancel",
    "pointerrawupdate",
    "gotpointercapture",
    "lostpointercapture",
    "touchstart",
    "touchend",
    "touchmove",
    "touchcancel",
    "drag",
    "dragstart",
    "dragend",
    "dragover",
    "dragenter",
    "dragleave",
    "dragexit",
    "drop",
    "keydown",
    "keypress",
    "keyup",
    "input",
    "beforeinput",
    "change",
    "submit",
    "reset",
    "select",
    "scroll",
    "wheel",
    "focus",
    "blur",
    "load",
    "error",
    "unload",
    "abort",
    "readystatechange",
    "propertychange",
    "canplay",
    "canplaythrough",
    "durationchange",
    "emptied",
    "ended",
    "loadeddata",
    "loadedmetadata",
    "loadstart",
    "pause",
    "play",
    "playing",
    "progress",
    "ratechange",
    "seeked",
    "seeking",
    "stalled",
    "suspend",
    "timeupdate",
    "volumechange",
    "waiting",
    "copy",
    "cut",
    "paste",
    "beforecopy",
    "beforecut",
    "beforepaste",
    "toggle",
    "animationstart",
    "transitionend",
];

static HTML_HANDLER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\bon(?:{})\s*=",
        HTML_HANDLER_NAMES.join("|")
    ))
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
    if lower.contains("javascript:") || lower.contains("vbscript:") || lower.contains("file:") {
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
pub(crate) const LINK_PREFIXES: &[&str] = &[
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

// === Secret-at-rest encryption (WebDAV password) ===

/// Marker prefix on a `webdav_password` value that was encrypted with DPAPI
/// before being stored. Legacy values without this prefix are plaintext
/// (pre-encryption installs) and are passed through unchanged until the next
/// `save_settings` re-encrypts them.
pub const DPAPI_PREFIX: &str = "dpapi:";

/// Encrypt a secret for storage using Windows DPAPI (`CryptProtectData`).
/// The resulting blob is scoped to the current user + machine, so a DB copied
/// to another account or machine cannot be decrypted there.
///
/// Non-Windows fallback stores the value as-is (this app is Windows-only; the
/// fallback exists so the crate still compiles off-Windows).
#[cfg(windows)]
pub fn encrypt_secret(plaintext: &str) -> Result<String, String> {
    use base64::Engine;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if plaintext.len() > u32::MAX as usize {
        return Err("secret too large to encrypt".into());
    }
    let bytes = plaintext.as_bytes();
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &data_in,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )
    };
    if ok == 0 {
        return Err("DPAPI 加密失败".into());
    }
    let cipher = unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) };
    let encoded = format!(
        "{}{}",
        DPAPI_PREFIX,
        base64::engine::general_purpose::STANDARD.encode(cipher)
    );
    unsafe { crate::ffi::LocalFree(data_out.pbData as *mut core::ffi::c_void) };
    Ok(encoded)
}

#[cfg(not(windows))]
pub fn encrypt_secret(plaintext: &str) -> Result<String, String> {
    Ok(plaintext.to_string())
}

/// Decrypt a DPAPI-encrypted secret (must carry [`DPAPI_PREFIX`]).
/// A value without the prefix is a legacy plaintext secret and is returned
/// unchanged so old databases keep working until re-saved.
#[cfg(windows)]
pub fn decrypt_secret(encoded: &str) -> Result<String, String> {
    use base64::Engine;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let Some(b64) = encoded.strip_prefix(DPAPI_PREFIX) else {
        // Legacy plaintext stored before encryption was introduced.
        return Ok(encoded.to_string());
    };
    let cipher = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("密文格式错误: {e}"))?;
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: cipher.len().min(u32::MAX as usize) as u32,
        pbData: cipher.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &data_in,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )
    };
    if ok == 0 {
        return Err("DPAPI 解密失败（数据库可能来自其他用户或机器）".into());
    }
    let plain = unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) };
    let text =
        String::from_utf8(plain.to_vec()).map_err(|e| format!("解密结果不是合法文本: {e}"))?;
    unsafe { crate::ffi::LocalFree(data_out.pbData as *mut core::ffi::c_void) };
    Ok(text)
}

#[cfg(not(windows))]
pub fn decrypt_secret(encoded: &str) -> Result<String, String> {
    Ok(encoded.to_string())
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
        assert_eq!(
            link_scheme("thunder://QUFodHRwOi8vZXhhbXBsZS5jb20v"),
            Some("thunder")
        );
        assert!(!is_openable_link("javascript:alert(1)"));
        assert!(!is_openable_link("data:text/html,hi"));
        assert!(!is_openable_link("file:///c:/x"));
        assert!(!is_openable_link("magnet:"));
        assert!(!is_openable_link("http://"));
        assert!(!is_openable_link("see magnet:?xt=urn:btih:abc more text"));
        assert!(!is_openable_link("plain text"));
        // http(s) subset still openable; download schemes are not "browser-only" but are openable
        assert!(matches!(link_scheme("https://a.com"), Some("https")));
        assert!(matches!(
            link_scheme("magnet:?xt=urn:btih:abc"),
            Some("magnet")
        ));
    }

    #[test]
    fn import_html_rejects_executable_markup() {
        assert!(is_safe_import_html("<p>hello <b>world</b></p>"));
        assert!(is_safe_import_html("<p>set one=2 and two=3</p>"));
        assert!(!is_safe_import_html("<p>x<script>alert(1)</script></p>"));
        assert!(!is_safe_import_html("<img src=x onerror=alert(1)>"));
        assert!(!is_safe_import_html(
            "<a href=\"javascript:alert(1)\">x</a>"
        ));
        assert!(!is_safe_import_html(
            "<iframe src=\"https://evil\"></iframe>"
        ));
        assert!(!is_safe_import_html("<svg onload=alert(1)>"));
    }

    #[test]
    fn import_html_rejects_full_handler_attr_names() {
        // Pointer / touch / drag / media handlers that were historically missing
        // from the blocklist — each must be rejected too.
        assert!(!is_safe_import_html("<div onpointerenter=alert(1)>x</div>"));
        assert!(!is_safe_import_html(
            "<div onpointerrawupdate=alert(1)>x</div>"
        ));
        assert!(!is_safe_import_html("<div ondragenter=alert(1)>x</div>"));
        assert!(!is_safe_import_html("<video oncanplay=alert(1)>"));
        assert!(!is_safe_import_html("<video onloadeddata=alert(1)>"));
        assert!(!is_safe_import_html("<div onpaste=alert(1)>x</div>"));
        assert!(!is_safe_import_html(
            "<div onanimationstart=alert(1)>x</div>"
        ));
        // Case-insensitivity + attribute spacing still honoured.
        assert!(!is_safe_import_html(
            "<div OnPointerEnter =alert(1)>x</div>"
        ));
        // Plain text that merely contains "on" words must stay safe.
        assert!(is_safe_import_html("<p>turn it on click here</p>"));
        assert!(is_safe_import_html("<p>consider progress done</p>"));
        assert!(is_safe_import_html("<p>set one=2 and two=3</p>"));
    }

    #[test]
    fn json_path_requires_json_ext() {
        assert!(validate_json_io_path("C:\\tmp\\out.txt", true).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_secret_round_trips() {
        let plain = "correct horse battery staple";
        let enc = encrypt_secret(plain).expect("encrypt");
        assert!(enc.starts_with(DPAPI_PREFIX));
        assert_eq!(decrypt_secret(&enc).expect("decrypt"), plain);
    }

    #[test]
    fn dpapi_legacy_plaintext_passes_through() {
        // Pre-encryption stored value (no prefix) must survive decrypt unchanged.
        assert_eq!(
            decrypt_secret("legacy-password").unwrap(),
            "legacy-password"
        );
    }
}
