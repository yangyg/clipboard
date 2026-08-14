//! `get_system_fonts`: enumerate OS-installed CJK-capable font families for
//! the UI font picker. Mirrors how Windows Terminal reads the system font
//! collection (DirectWrite behind `font-kit`), filtered to families that can
//! render Chinese, ordered common-first then alphabetically.
//!
//! Lazily enumerated once and cached in memory — the settings page triggers the
//! first call, so cold-start is unaffected.

use font_kit::source::SystemSource;
use std::collections::HashSet;
use std::sync::Mutex;

static FONT_CACHE: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Common CJK families float to the top of the picker when installed.
/// Matched case-insensitively against the family name (CJK spellings included).
const COMMON_CJK_FRAGMENTS: &[&str] = &[
    "microsoft yahei",
    "微软雅黑",
    "simhei",
    "黑体",
    "simsun",
    "宋体",
    "kaiti",
    "楷体",
    "dengxian",
    "等线",
    "noto sans sc",
    "noto serif sc",
];

/// Installed CJK-capable font families (cached after the first successful call).
/// Async: the enumeration reads every installed font face, which takes a few
/// seconds — run it off the main thread (via `spawn_blocking`) so the UI never
/// freezes while the settings picker loads.
///
/// Returns `Err` when DirectWrite enumeration fails so the settings page can
/// distinguish "load failed" from "no CJK fonts installed".
#[tauri::command]
pub async fn get_system_fonts() -> Result<Vec<String>, String> {
    {
        let guard = FONT_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }
    }
    let fonts = tauri::async_runtime::spawn_blocking(enumerate_cjk_fonts)
        .await
        .map_err(|e| format!("枚举系统字体失败: {e}"))??;
    let mut guard = FONT_CACHE.lock().map_err(|e| e.to_string())?;
    *guard = Some(fonts.clone());
    Ok(fonts)
}

fn enumerate_cjk_fonts() -> Result<Vec<String>, String> {
    let source = SystemSource::new();
    let families = source
        .all_families()
        .map_err(|e| format!("读取系统字体失败: {e}"))?;

    let mut seen = HashSet::new();
    let mut common = Vec::new();
    let mut rest = Vec::new();

    for name in families {
        if !seen.insert(name.clone()) {
            continue;
        }
        // CJK coverage: the family must actually contain a CJK glyph. Only the
        // first face needs loading — a family either covers CJK or it doesn't.
        let supports_cjk = source
            .select_family_by_name(&name)
            .ok()
            .and_then(|handle| handle.fonts().first().cloned())
            .and_then(|h| h.load().ok())
            .is_some_and(|font| font.glyph_for_char('\u{4E00}').is_some());
        if !supports_cjk {
            continue;
        }

        let key = name.to_lowercase();
        if COMMON_CJK_FRAGMENTS.iter().any(|f| key.contains(f)) {
            common.push(name);
        } else {
            rest.push(name);
        }
    }

    common.sort_by_key(|a| a.to_lowercase());
    rest.sort_by_key(|a| a.to_lowercase());
    Ok(common.into_iter().chain(rest).collect())
}

#[cfg(test)]
mod tests {
    use super::{enumerate_cjk_fonts, COMMON_CJK_FRAGMENTS};

    #[test]
    fn common_fragments_are_lowercase_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in COMMON_CJK_FRAGMENTS {
            assert_eq!(f.to_lowercase(), *f, "fragment should be lowercase: {f}");
            assert!(seen.insert(*f), "duplicate fragment: {f}");
        }
    }

    #[test]
    fn enumeration_returns_unique_sorted_names() {
        let fonts = enumerate_cjk_fonts().expect("enumerate system fonts");
        let mut seen = std::collections::HashSet::new();
        for name in &fonts {
            assert!(!name.is_empty());
            assert!(seen.insert(name.clone()), "duplicate font: {name}");
        }
        // Order: common block then rest, each sorted case-insensitively.
        let splits: Vec<&[String]> = fonts
            .split(|n| {
                !COMMON_CJK_FRAGMENTS
                    .iter()
                    .any(|f| n.to_lowercase().contains(f))
            })
            .filter(|s| !s.is_empty())
            .collect();
        for group in splits {
            let mut sorted = group.to_vec();
            sorted.sort_by_key(|a| a.to_lowercase());
            assert_eq!(group, sorted.as_slice(), "group not sorted: {group:?}");
        }
    }
}
