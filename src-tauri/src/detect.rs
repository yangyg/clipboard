//! Pure content-classification & fingerprint helpers.
//!
//! Kept free of Tauri/DB state so they can be unit-tested in isolation.

use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::types::ContentType;

// ============================================================
// Pre-compiled Regex Patterns
// ============================================================

static CODE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"^\s*(fn|function|def|class|struct|impl|pub|const|let|var|import|from|require|module)\s"#,
        r#"^\s*(if|for|while|switch|match)\s*[({]"#,
        r#"^\s*(const|let|var)\s+\w+\s*[=:>]"#,
        r#"^\s*#[a-z]+\s"#,
        r#"\{\s*["']\w+["']\s*:"#,
        r#"</?[a-z][a-z0-9]*"#,
        r#"^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)\s"#,
        r#"^\s*---\s*$"#,
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

// Digit-run detection uses *maximal* ASCII digit runs instead of `\b…\b`:
// Rust regex word boundaries are Unicode-aware, so CJK characters adjacent to
// digits (e.g. 「验证码为837261」) defeat `\b` and leak OTPs. A run of exactly
// N digits is delimiter-independent and still rejects longer digit blobs
// (e.g. a 20-digit id is not a 4-8 digit code).
static DIGIT_RUN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u)\d+").unwrap());
static API_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bsk-[a-zA-Z0-9_-]{20,}").unwrap());
// JWT: three base64url segments (header always starts with "eyJ").
static JWT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)eyJ[a-zA-Z0-9_-]{4,}\.[a-zA-Z0-9_-]{4,}\.[a-zA-Z0-9_-]{4,}").unwrap()
});
// GitHub personal access / OAuth / server-to-server / fine-grained tokens.
static GITHUB_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)\b(gh[pousr]_[a-zA-Z0-9]{20,}|github_pat_[a-zA-Z0-9_]{20,})").unwrap()
});
static AWS_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bAKIA[0-9A-Z]{16}").unwrap());
static GOOGLE_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bAIza[0-9A-Za-z_-]{35}").unwrap());
static SLACK_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bxox[baprs]-[a-zA-Z0-9-]{10,}").unwrap());
// Stripe secret keys use underscores — the `sk-` rule above does not cover them.
static STRIPE_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bsk_(live|test)_[a-zA-Z0-9]{10,}").unwrap());
static PEM_PRIVATE_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----").unwrap());
/// Password-like keywords (word-boundary). A username like `mypassword123`,
/// `upwd`, or a bare `pwd` shell command must not trip the detector.
static PASSWORD_KEYWORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:password|passwd)s?\b").unwrap());
/// `pwd` is only a credential when it carries an assignment marker
/// (`pwd=…` / `pwd: …`), so shell usage and prose stay unflagged.
static PWD_ASSIGN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bpwd\b\s*[:=：]").unwrap());
/// Chinese password keywords need the same assignment marker — "重置密码" /
/// "密码规则" documentation must not auto-expire a record.
static CN_PASSWORD_ASSIGN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(密码|口令)\s*[:=：]").unwrap());
/// Verification-code keywords: only *qualified* "code" mentions count
/// (verification / OTP / auth / security / access / sms / one-time / 2FA),
/// plus standalone `otp` / `2fa`. "zip code 10001" and "promo code 123456"
/// are not verification codes.
static CODE_OR_OTP_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:verification|verify|otp|one[ -]?time|2fa|two[ -]?factor|auth(?:entication)?|security|access|sms)[ -]?(?:code|pin)\b|\b(?:otp|2fa)\b",
    )
    .unwrap()
});

/// Length ceilings for the digit-based heuristics. Verification codes / card
/// numbers arrive in short messages; a long document that merely contains
/// digits should not be flagged as sensitive.
const VERIFICATION_MAX_CHARS: usize = 60;
const BANK_CARD_MAX_CHARS: usize = 25;
/// Password keywords only flag short snippets ("password: xxx" notes). Long
/// source/config/doc copies that merely mention the word must not be
/// auto-expired — a false positive here silently destroys user data.
const PASSWORD_KEYWORD_MAX_CHARS: usize = 200;

/// True if the content contains a maximal ASCII digit run whose byte length
/// (== char count, digits are single-byte) falls in `min..=max`.
fn has_digit_run(content: &str, min: usize, max: usize) -> bool {
    DIGIT_RUN_RE
        .find_iter(content)
        .any(|m| (min..=max).contains(&(m.end() - m.start())))
}

// ============================================================
// Classification
// ============================================================

pub fn detect_content_type(content: &str) -> ContentType {
    let trimmed = content.trim();

    // Link / download URI (http(s), ftp, magnet, ed2k, thunder)
    if crate::security::is_openable_link(trimmed) {
        return ContentType::Link;
    }

    // File path heuristic only — avoid Path::exists() disk IO on the monitor thread
    if trimmed.len() < 2048
        && ((trimmed.contains(":\\") && trimmed.contains('.'))
            || (trimmed.starts_with('/') && trimmed.contains('.')))
    {
        return ContentType::File;
    }

    // Code detection (pre-compiled patterns)
    for re in CODE_PATTERNS.iter() {
        if re.is_match(trimmed) {
            return ContentType::Code;
        }
    }

    // JSON detection — avoid full parse of huge clipboard payloads
    const JSON_DETECT_MAX: usize = 64 * 1024;
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if trimmed.len() <= JSON_DETECT_MAX {
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                return ContentType::Code;
            }
        } else {
            // Oversized: treat brace-wrapped blobs as code without parsing.
            return ContentType::Code;
        }
    }

    ContentType::Text
}

/// Heuristic sensitive-content detector (text only).
///
/// Digit-based rules are gated by both a *delimited* digit run and a message
/// length ceiling so that source files / long text are not mislabelled (and
/// therefore not auto-expired).
pub fn detect_sensitive(content: &str) -> bool {
    let trimmed = content.trim();
    let lower = trimmed.to_lowercase();
    let char_count = trimmed.chars().count();

    // Password-like keywords — short snippets only (see cap doc above).
    if char_count <= PASSWORD_KEYWORD_MAX_CHARS {
        // URL-ish payloads (e.g. a password-reset link) are not secrets
        // themselves; flagging them would auto-expire the user's link.
        let link_like = trimmed.contains("://") || crate::security::is_openable_link(trimmed);
        if !link_like
            && (PASSWORD_KEYWORD_RE.is_match(trimmed)
                || PWD_ASSIGN_RE.is_match(trimmed)
                || CN_PASSWORD_ASSIGN_RE.is_match(trimmed))
        {
            return true;
        }
    }

    // Credential formats: API keys, JWTs, platform tokens, private keys.
    if API_KEY_RE.is_match(trimmed)
        || JWT_RE.is_match(trimmed)
        || GITHUB_TOKEN_RE.is_match(trimmed)
        || AWS_KEY_RE.is_match(trimmed)
        || GOOGLE_KEY_RE.is_match(trimmed)
        || SLACK_TOKEN_RE.is_match(trimmed)
        || STRIPE_KEY_RE.is_match(trimmed)
        || PEM_PRIVATE_KEY_RE.is_match(trimmed)
    {
        return true;
    }

    // Verification codes: a standalone 4-8 digit run in a short message that
    // also mentions a verification keyword.
    if char_count <= VERIFICATION_MAX_CHARS
        && has_digit_run(trimmed, 4, 8)
        && (trimmed.contains("验证码")
            || trimmed.contains("校验码")
            || trimmed.contains("动态口令")
            || CODE_OR_OTP_WORD_RE.is_match(&lower))
    {
        return true;
    }

    // Bank card numbers: a 16-19 digit run in a short string.
    if char_count <= BANK_CARD_MAX_CHARS && has_digit_run(trimmed, 16, 19) {
        return true;
    }

    false
}

pub fn sha256_hash(content: &str) -> String {
    sha256_hash_bytes(content.as_bytes())
}

pub fn sha256_hash_bytes(bytes: &[u8]) -> String {
    sha256_hash_slices(&[bytes])
}

/// SHA-256 over multiple byte slices in one pass (used for combined payloads).
pub fn sha256_hash_slices(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}

/// True when a capture's stored payload would exceed `max_text_bytes`.
/// `cap == 0` means unlimited. HTML is counted with the plain text so a tiny
/// caption plus a huge CF_HTML fragment cannot bypass the storage cap.
pub fn exceeds_text_byte_cap(text_len: usize, html_len: usize, cap: i32) -> bool {
    let cap = cap.max(0) as usize;
    cap > 0 && text_len.saturating_add(html_len) > cap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_detected() {
        assert_eq!(
            detect_content_type("https://example.com"),
            ContentType::Link
        );
        assert_eq!(detect_content_type("  ftp://host/file "), ContentType::Link);
        assert_eq!(
            detect_content_type("HTTP://Example.COM/a"),
            ContentType::Link
        );
        assert_eq!(
            detect_content_type("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"),
            ContentType::Link
        );
        assert_eq!(
            detect_content_type("ed2k://|file|name.iso|123|ABCDEF0123456789ABCDEF0123456789|/"),
            ContentType::Link
        );
        assert_eq!(
            detect_content_type("thunder://QUFodHRwOi8vZXhhbXBsZS5jb20v"),
            ContentType::Link
        );
        assert_eq!(
            detect_content_type("download this magnet:?xt=urn:btih:abc please"),
            ContentType::Text
        );
        assert_eq!(
            detect_content_type("javascript:alert(1)"),
            ContentType::Text
        );
    }

    #[test]
    fn file_paths_are_detected() {
        assert_eq!(
            detect_content_type(r"C:\Users\a\report.pdf"),
            ContentType::File
        );
        assert_eq!(
            detect_content_type("/usr/local/bin/tool.sh"),
            ContentType::File
        );
    }

    #[test]
    fn code_and_json_are_detected() {
        assert_eq!(detect_content_type("fn main() {}"), ContentType::Code);
        assert_eq!(detect_content_type("SELECT * FROM t"), ContentType::Code);
        assert_eq!(detect_content_type(r#"{"a": 1}"#), ContentType::Code);
    }

    #[test]
    fn plain_text_is_default() {
        assert_eq!(
            detect_content_type("just a normal sentence"),
            ContentType::Text
        );
        assert_eq!(
            detect_content_type("你好，这是一段普通文本"),
            ContentType::Text
        );
    }

    #[test]
    fn password_keywords_are_sensitive() {
        assert!(detect_sensitive("my password is hunter2"));
        assert!(detect_sensitive("PWD=secret"));
        assert!(detect_sensitive("登录密码：hunter2"));
    }

    #[test]
    fn api_keys_are_sensitive() {
        assert!(detect_sensitive(
            "token sk-abcdefghijklmnopqrstuvwxyz012345"
        ));
    }

    #[test]
    fn verification_code_needs_keyword_and_short_message() {
        assert!(detect_sensitive("您的验证码是 837261，请勿泄露"));
        // No space around the digits — CJK chars must not defeat detection.
        assert!(detect_sensitive("您的验证码为837261请勿泄露"));
        assert!(detect_sensitive("您的动态口令为837261请勿泄露"));
        assert!(detect_sensitive("Your verification code: 123456"));
        assert!(detect_sensitive("Your OTP is 123456"));
        // Digits without a keyword are not sensitive.
        assert!(!detect_sensitive("订单号 837261 已发货"));
        // Unqualified "code" mentions (zip / promo) are not verification codes.
        assert!(!detect_sensitive("zip code 10001"));
        assert!(!detect_sensitive("promo code 123456"));
        assert!(!detect_sensitive("Your code: 123456"));
    }

    #[test]
    fn code_keyword_needs_word_boundary() {
        // Substring matches (barcode/zipcode) must not flag content.
        assert!(!detect_sensitive("barcode 940123"));
        assert!(!detect_sensitive("zipcode 10001"));
    }

    #[test]
    fn password_keyword_requires_short_text() {
        assert!(detect_sensitive("my password is hunter2"));
        // A long document merely mentioning the word is not a password snippet.
        let mut long = String::from("password ");
        long.push_str(&"x".repeat(300));
        assert!(!detect_sensitive(&long));
    }

    #[test]
    fn password_keyword_requires_word_boundary() {
        // Embedded words / bare shell commands must not auto-expire.
        assert!(!detect_sensitive("mypassword123"));
        assert!(!detect_sensitive("upwd"));
        assert!(!detect_sensitive("pwd"));
        assert!(!detect_sensitive("run pwd to see the directory"));
    }

    #[test]
    fn password_reset_links_are_not_sensitive() {
        // A reset link contains "password" and a digit run — flagging it would
        // auto-expire the very link the user is trying to keep.
        assert!(!detect_sensitive(
            "https://example.com/reset-password?id=123456"
        ));
    }

    #[test]
    fn credential_formats_are_sensitive() {
        // OpenAI project-style keys contain an inner hyphen.
        assert!(detect_sensitive("sk-proj-abcdefghijklmnopqrstuvwxyz012345"));
        assert!(detect_sensitive("ghp_ABCDEFghijkl0123456789mnopQRSTUVWXyz"));
        assert!(detect_sensitive("AKIAIOSFODNN7EXAMPLE"));
        assert!(detect_sensitive(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        ));
        assert!(detect_sensitive("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(detect_sensitive("xoxb-123456789012-abcdef"));
        assert!(detect_sensitive("sk_live_51ABCdefGHIjklMNOpqr"));
        // Hyphenated prose containing "sk-" (e.g. "task-…") is not a key.
        assert!(!detect_sensitive(
            "task-management-system-version-two-notes"
        ));
    }

    #[test]
    fn code_files_are_not_flagged_as_verification() {
        // Contains the word "code" but no standalone 4-8 digit group.
        assert!(!detect_sensitive("let code = 42;"));
    }

    #[test]
    fn bank_card_number_delimited_only() {
        assert!(detect_sensitive("6222021234567890123"));
        // A longer digit run (not a delimited 16-19 group) is not a card.
        assert!(!detect_sensitive("123456789012345678901234567890"));
    }

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn text_byte_cap_counts_html_and_treats_zero_as_unlimited() {
        assert!(!exceeds_text_byte_cap(10, 0, 0));
        assert!(!exceeds_text_byte_cap(10, 0, 10));
        assert!(exceeds_text_byte_cap(11, 0, 10));
        assert!(exceeds_text_byte_cap(4, 7, 10));
        assert!(!exceeds_text_byte_cap(4, 6, 10));
    }
}
