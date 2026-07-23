//! Pure content-classification & fingerprint helpers.
//!
//! Kept free of Tauri/DB state so they can be unit-tested in isolation.

use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::db::ContentType;

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

// `\b…\b` keeps the digit run *delimited*: a longer number (e.g. a 20-digit id)
// no longer matches, cutting false positives on ordinary numeric content.
static VERIFICATION_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4,8}\b").unwrap());
static API_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap());
static BANK_CARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{16,19}\b").unwrap());

/// Length ceilings for the digit-based heuristics. Verification codes / card
/// numbers arrive in short messages; a long document that merely contains
/// digits should not be flagged as sensitive.
const VERIFICATION_MAX_CHARS: usize = 60;
const BANK_CARD_MAX_CHARS: usize = 25;

// ============================================================
// Classification
// ============================================================

pub fn detect_content_type(content: &str) -> ContentType {
    let trimmed = content.trim();

    // URL detection
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("ftp://") {
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

    // Password-like keywords
    if lower.contains("password") || lower.contains("passwd") || lower.contains("pwd") {
        return true;
    }

    // API keys (sk-...)
    if API_KEY_RE.is_match(trimmed) {
        return true;
    }

    let char_count = trimmed.chars().count();

    // Verification codes: a standalone 4-8 digit group in a short message that
    // also mentions a verification keyword.
    if char_count <= VERIFICATION_MAX_CHARS
        && VERIFICATION_CODE_RE.is_match(trimmed)
        && (trimmed.contains("验证码")
            || lower.contains("code")
            || lower.contains("verification")
            || lower.contains("otp"))
    {
        return true;
    }

    // Bank card numbers: a delimited 16-19 digit group in a short string.
    if char_count <= BANK_CARD_MAX_CHARS && BANK_CARD_RE.is_match(trimmed) {
        return true;
    }

    false
}

pub fn sha256_hash(content: &str) -> String {
    sha256_hash_bytes(content.as_bytes())
}

pub fn sha256_hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_detected() {
        assert_eq!(detect_content_type("https://example.com"), ContentType::Link);
        assert_eq!(detect_content_type("  ftp://host/file "), ContentType::Link);
    }

    #[test]
    fn file_paths_are_detected() {
        assert_eq!(detect_content_type(r"C:\Users\a\report.pdf"), ContentType::File);
        assert_eq!(detect_content_type("/usr/local/bin/tool.sh"), ContentType::File);
    }

    #[test]
    fn code_and_json_are_detected() {
        assert_eq!(detect_content_type("fn main() {}"), ContentType::Code);
        assert_eq!(detect_content_type("SELECT * FROM t"), ContentType::Code);
        assert_eq!(detect_content_type(r#"{"a": 1}"#), ContentType::Code);
    }

    #[test]
    fn plain_text_is_default() {
        assert_eq!(detect_content_type("just a normal sentence"), ContentType::Text);
        assert_eq!(detect_content_type("你好，这是一段普通文本"), ContentType::Text);
    }

    #[test]
    fn password_keywords_are_sensitive() {
        assert!(detect_sensitive("my password is hunter2"));
        assert!(detect_sensitive("PWD=secret"));
    }

    #[test]
    fn api_keys_are_sensitive() {
        assert!(detect_sensitive("token sk-abcdefghijklmnopqrstuvwxyz012345"));
    }

    #[test]
    fn verification_code_needs_keyword_and_short_message() {
        assert!(detect_sensitive("您的验证码是 837261，请勿泄露"));
        assert!(detect_sensitive("Your code: 123456"));
        // Digits without a keyword are not sensitive.
        assert!(!detect_sensitive("订单号 837261 已发货"));
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
}
