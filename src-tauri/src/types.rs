//! Shared IPC / persistence types (extracted from lib.rs to keep the app-entry
//! file small). The structs mirror `src/types.ts` 1:1 — serde field names are
//! the IPC contract and must not drift.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::clipboard::ClipboardMonitor;
use crate::db::ClipboardDb;
use crate::features::FeatureFlags;

// === App State ===
pub struct AppState {
    pub db: Arc<ClipboardDb>,
    pub monitor: Arc<RwLock<ClipboardMonitor>>,
    pub capture_paused: Arc<RwLock<bool>>,
}

// === Tauri Record Type (must match src/types.ts ClipboardRecord) ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    #[serde(rename = "source_app")]
    pub source_app: String,
    #[serde(rename = "source_window")]
    pub source_window: String,
    /// Friendly source name from the exe's FileDescription (display only; empty = fall back).
    #[serde(default, rename = "source_name")]
    pub source_name: String,
    /// Device that first captured this record (empty = legacy/unknown origin).
    /// Never overwritten by merges/re-copies so synced content keeps its origin.
    #[serde(default, rename = "source_device_id")]
    pub source_device_id: String,
    pub hash: String,
    #[serde(rename = "copy_count")]
    pub copy_count: i32,
    #[serde(rename = "is_favorite")]
    pub is_favorite: bool,
    #[serde(rename = "is_pinned")]
    pub is_pinned: bool,
    #[serde(rename = "is_sensitive")]
    pub is_sensitive: bool,
    #[serde(rename = "is_trashed")]
    pub is_trashed: bool,
    #[serde(rename = "auto_expire_at")]
    pub auto_expire_at: Option<String>,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Tag name → palette-color pairs carried by the sync/export bundle so tag
    /// colors follow records across devices. Empty outside the export path.
    #[serde(default, rename = "tag_colors", skip_serializing_if = "Vec::is_empty")]
    pub tag_colors: Vec<(String, String)>,
    /// HTML clipboard fragment when format was captured (Word, browser, etc.)
    #[serde(default, rename = "content_html")]
    pub content_html: Option<String>,
    /// Relative path under app data dir, e.g. media/{hash}.png
    #[serde(default, rename = "media_path")]
    pub media_path: Option<String>,
    #[serde(default, rename = "thumb_path")]
    pub thumb_path: Option<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    /// Absolute filesystem paths for frontend convertFileSrc
    #[serde(default, rename = "media_abs")]
    pub media_abs: Option<String>,
    #[serde(default, rename = "thumb_abs")]
    pub thumb_abs: Option<String>,
    /// Full content character length (list rows may truncate `content`)
    #[serde(default, rename = "content_len")]
    pub content_len: Option<i32>,
    /// Short display alias (does not change paste content / hash). Empty = none.
    #[serde(default)]
    pub alias: String,
}

// === Settings (must match src/types.ts Settings) ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTagRule {
    #[serde(rename = "tag_name")]
    pub tag_name: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default, rename = "content_types")]
    pub content_types: Vec<String>,
}

/// User-defined source display-name override (matches the exe basename).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceNameOverride {
    #[serde(rename = "exe_name")]
    pub exe_name: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
}

fn default_enable_auto_tag() -> bool {
    true
}

/// Missing field in saved JSON → treat as already onboarded (upgrades).
fn default_onboarding_completed() -> bool {
    true
}

fn default_webdav_remote_path() -> String {
    "ClipVaultSync".to_string()
}

/// Missing `blur_strength` in saved JSON (upgrade from pre-0.x) → 45%.
fn default_blur_strength() -> i32 {
    45
}

fn default_language() -> String {
    "system".to_string()
}

/// AI model access is opt-in (off until the user fills the settings form).
fn default_enable_ai() -> bool {
    false
}

fn default_ai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_ai_model() -> String {
    "gpt-4o-mini".to_string()
}

/// Cap the snippet sent to the model — deep code / long emails don't need
/// full round-trips for a summary + tags.
fn default_ai_max_chars() -> i32 {
    4000
}

/// Skip enrichment for short content — a one-liner needs no summary. `0`
/// disables the floor.
fn default_ai_min_chars() -> i32 {
    32
}

/// Skip captures whose text exceeds this byte limit (0 = unlimited). Oversized
/// single copies otherwise bloat the DB *and* the FTS trigram index, and stall
/// the write lock while the index builds.
fn default_max_text_bytes() -> i32 {
    10 * 1024 * 1024
}

fn default_ai_summary_alias() -> bool {
    true
}

fn default_ai_auto_tag() -> bool {
    true
}

/// Annotation the model must return for a record: a short display alias plus
/// a handful of tags. `summary` may be empty (nothing worth summarizing).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiResult {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Missing `font_family` in saved JSON (upgrade) → the "default" preset.
/// Keep in sync with `FONT_PRESETS[0].key` in src/utils/fontPresets.ts.
fn default_font_family() -> String {
    "default".to_string()
}

/// Missing `search_mode` in saved JSON (upgrade) → full search box.
/// Keep in sync with `DEFAULT_SETTINGS.search_mode` in src/stores/settings.ts.
fn default_search_mode() -> String {
    "full".to_string()
}

/// L-3: Default auto-tag rules for new installs.
/// IMPORTANT: Keep in sync with `DEFAULT_AUTO_TAG_RULES` in src/types.ts.
pub fn default_auto_tag_rules() -> Vec<AutoTagRule> {
    vec![
        AutoTagRule {
            tag_name: "链接".to_string(),
            keywords: vec![],
            content_types: vec!["link".to_string()],
        },
        AutoTagRule {
            tag_name: "部署".to_string(),
            keywords: vec![
                "deploy".into(),
                "kubectl".into(),
                "docker".into(),
                "helm".into(),
                "k8s".into(),
                "npm run build".into(),
                "生产环境".into(),
            ],
            content_types: vec![],
        },
        AutoTagRule {
            tag_name: "前端".to_string(),
            keywords: vec![
                "vue".into(),
                "react".into(),
                "typescript".into(),
                "tsx".into(),
                "vite".into(),
                "webpack".into(),
                "frontend".into(),
                "前端".into(),
            ],
            content_types: vec![],
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "global_shortcut")]
    pub global_shortcut: String,
    #[serde(rename = "max_records")]
    pub max_records: i32,
    #[serde(rename = "retention_days")]
    pub retention_days: i32,
    pub theme: String,
    #[serde(rename = "panel_opacity")]
    pub panel_opacity: i32,
    #[serde(rename = "panel_radius")]
    pub panel_radius: i32,
    #[serde(rename = "enable_blur")]
    pub enable_blur: bool,
    /// Frosted-glass intensity (0-100): how much blurred desktop shows through
    /// when 毛玻璃 is on. Surface tint opacity = 100 - blur_strength.
    #[serde(default = "default_blur_strength", rename = "blur_strength")]
    pub blur_strength: i32,
    #[serde(rename = "enable_animation")]
    pub enable_animation: bool,
    #[serde(rename = "font_size")]
    pub font_size: i32,
    /// UI font-family preset key, or `system:<name>` for an OS-installed font.
    #[serde(default = "default_font_family", rename = "font_family")]
    pub font_family: String,
    /// Search bar display mode (`full` | `icon` | `hidden`), shared by the
    /// title bar and the list.
    #[serde(default = "default_search_mode", rename = "search_mode")]
    pub search_mode: String,
    /// Keep the window on top of other apps (single window mode).
    #[serde(default, rename = "always_on_top")]
    pub always_on_top: bool,
    #[serde(rename = "default_paste_mode")]
    pub default_paste_mode: String,
    #[serde(rename = "auto_close_on_paste")]
    pub auto_close_on_paste: bool,
    #[serde(rename = "enable_sensitive_detection")]
    pub enable_sensitive_detection: bool,
    #[serde(rename = "sensitive_auto_expire_seconds")]
    pub sensitive_auto_expire_seconds: i32,
    /// Max captured text size in bytes (0 = unlimited); larger copies are
    /// skipped entirely instead of being stored.
    #[serde(default = "default_max_text_bytes", rename = "max_text_bytes")]
    pub max_text_bytes: i32,
    /// On startup (first window show of the session), import the OS clipboard
    /// history items captured while the app was not running. Default off.
    /// See `win_history.rs` (Windows 11 clipboard history via WinRT).
    #[serde(default, rename = "import_system_history_on_start")]
    pub import_system_history_on_start: bool,
    #[serde(rename = "auto_start")]
    pub auto_start: bool,
    #[serde(rename = "minimize_to_tray")]
    pub minimize_to_tray: bool,
    #[serde(rename = "ignored_apps")]
    pub ignored_apps: Vec<String>,
    /// User-defined exe → display-name overrides (frontend-only resolution).
    #[serde(default, rename = "source_name_overrides")]
    pub source_name_overrides: Vec<SourceNameOverride>,
    /// Remembered logical size (0 = use adaptive default).
    #[serde(default, rename = "window_width")]
    pub window_width: i32,
    #[serde(default, rename = "window_height")]
    pub window_height: i32,
    #[serde(default = "default_enable_auto_tag", rename = "enable_auto_tag")]
    pub enable_auto_tag: bool,
    #[serde(default = "default_auto_tag_rules", rename = "auto_tag_rules")]
    pub auto_tag_rules: Vec<AutoTagRule>,
    #[serde(
        default = "default_onboarding_completed",
        rename = "onboarding_completed"
    )]
    pub onboarding_completed: bool,
    // --- UI language preference ---
    #[serde(default = "default_language", rename = "language")]
    pub language: String,
    // --- WebDAV sync (credentials stay local; never part of JSON export) ---
    #[serde(default, rename = "webdav_url")]
    pub webdav_url: String,
    #[serde(default, rename = "webdav_username")]
    pub webdav_username: String,
    #[serde(default, rename = "webdav_password")]
    pub webdav_password: String,
    #[serde(default = "default_webdav_remote_path", rename = "webdav_remote_path")]
    pub webdav_remote_path: String,
    #[serde(default, rename = "webdav_sync_sensitive")]
    pub webdav_sync_sensitive: bool,
    #[serde(default, rename = "webdav_device_id")]
    pub webdav_device_id: String,
    /// Display name for this device, published in the sync manifest so other
    /// devices can label records that originated here.
    #[serde(default, rename = "webdav_device_name")]
    pub webdav_device_name: String,
    /// device_id → display name learned from sync manifests (local cache).
    #[serde(default, rename = "webdav_device_names")]
    pub webdav_device_names: std::collections::HashMap<String, String>,
    #[serde(default, rename = "webdav_last_sync_at")]
    pub webdav_last_sync_at: Option<String>,
    // --- AI enrichment (OpenAI-compatible chat completions) ---
    #[serde(default = "default_enable_ai", rename = "enable_ai")]
    pub enable_ai: bool,
    #[serde(default = "default_ai_base_url", rename = "ai_base_url")]
    pub ai_base_url: String,
    /// DPAPI-encrypted at rest (mirrors `webdav_password`); kept plaintext in memory.
    #[serde(default, rename = "ai_api_key")]
    pub ai_api_key: String,
    #[serde(default = "default_ai_model", rename = "ai_model")]
    pub ai_model: String,
    #[serde(default = "default_ai_summary_alias", rename = "ai_summary_alias")]
    pub ai_summary_alias: bool,
    #[serde(default = "default_ai_auto_tag", rename = "ai_auto_tag")]
    pub ai_auto_tag: bool,
    /// Content truncation before it leaves the machine (chars).
    #[serde(default = "default_ai_max_chars", rename = "ai_max_chars")]
    pub ai_max_chars: i32,
    /// Skip enrichment for shorter content (chars); 0 = no floor.
    #[serde(default = "default_ai_min_chars", rename = "ai_min_chars")]
    pub ai_min_chars: i32,
    /// Optional product capabilities (tags / batch / sync / stats). Missing → all on.
    #[serde(default, rename = "features")]
    pub features: FeatureFlags,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            global_shortcut: "Ctrl+Shift+V".to_string(),
            max_records: 1000,
            retention_days: 30,
            theme: "dark".to_string(),
            panel_opacity: 94,
            panel_radius: 20,
            enable_blur: false,
            blur_strength: 45,
            enable_animation: true,
            font_size: 16,
            font_family: default_font_family(),
            search_mode: default_search_mode(),
            always_on_top: false,
            default_paste_mode: "original".to_string(),
            auto_close_on_paste: true,
            enable_sensitive_detection: true,
            sensitive_auto_expire_seconds: 600,
            max_text_bytes: default_max_text_bytes(),
            import_system_history_on_start: false,
            auto_start: false,
            minimize_to_tray: true,
            ignored_apps: vec![
                "1Password.exe".to_string(),
                "Bitwarden.exe".to_string(),
                "KeePass.exe".to_string(),
                "KeePassXC.exe".to_string(),
                "Enpass.exe".to_string(),
                "Dashlane.exe".to_string(),
                "ICBCNetBank.exe".to_string(),
            ],
            source_name_overrides: vec![],
            window_width: 0,
            window_height: 0,
            enable_auto_tag: true,
            auto_tag_rules: default_auto_tag_rules(),
            onboarding_completed: false,
            language: default_language(),
            webdav_url: String::new(),
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_remote_path: default_webdav_remote_path(),
            webdav_sync_sensitive: false,
            webdav_device_id: String::new(),
            webdav_device_name: String::new(),
            webdav_device_names: std::collections::HashMap::new(),
            webdav_last_sync_at: None,
            enable_ai: false,
            ai_base_url: default_ai_base_url(),
            ai_api_key: String::new(),
            ai_model: default_ai_model(),
            ai_summary_alias: true,
            ai_auto_tag: true,
            ai_max_chars: default_ai_max_chars(),
            ai_min_chars: default_ai_min_chars(),
            features: FeatureFlags::default(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub records: Vec<ClipboardRecord>,
    pub total: usize,
    pub query: String,
    #[serde(rename = "elapsed_ms")]
    pub elapsed_ms: u64,
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct RecordsPage {
    pub records: Vec<ClipboardRecord>,
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

/// One distinct search-history entry (autocomplete + future stats).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryEntry {
    #[serde(rename = "search_count")]
    pub search_count: i64,
    #[serde(rename = "last_searched_at")]
    pub last_searched_at: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub is_auto: bool,
    pub count: i64,
}

/// One WebDAV sync operation log row (local-only, never synced).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncHistoryEntry {
    pub id: i64,
    pub synced_at: String,
    /// `pull` | `push` | `sync`
    pub action: String,
    pub success: bool,
    pub pulled: i32,
    pub pushed: i32,
    pub merged: i32,
    pub tags_pulled: i32,
    pub tags_pushed: i32,
    pub media_downloaded: i32,
    pub media_uploaded: i32,
    pub media_skipped: i32,
    /// Failure detail (success entries carry `None`).
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsData {
    #[serde(rename = "total_records")]
    pub total_records: i64,
    #[serde(rename = "total_copies")]
    pub total_copies: i64,
    #[serde(rename = "favorites_count")]
    pub favorites_count: i64,
    #[serde(rename = "pinned_count")]
    pub pinned_count: i64,
    #[serde(rename = "sensitive_count")]
    pub sensitive_count: i64,
    #[serde(rename = "storage_bytes")]
    pub storage_bytes: i64,
    /// Absolute path to app data dir (DB + media).
    #[serde(rename = "data_path")]
    pub data_path: String,
    #[serde(rename = "type_distribution")]
    pub type_distribution: std::collections::HashMap<String, i64>,
}

#[cfg(test)]
mod settings_onboarding_tests {
    use super::Settings;

    #[test]
    fn default_settings_needs_onboarding() {
        assert!(!Settings::default().onboarding_completed);
    }

    #[test]
    fn ai_enrichment_off_by_default() {
        let s = Settings::default();
        assert!(!s.enable_ai);
        assert!(s.ai_summary_alias);
        assert!(s.ai_auto_tag);
        assert_eq!(s.ai_max_chars, 4000);
        assert_eq!(s.ai_min_chars, 32);
        assert!(!s.ai_base_url.is_empty());
    }

    #[test]
    fn missing_ai_fields_survive_upgrade_json() {
        let json = r#"{"global_shortcut":"Ctrl+Shift+V","max_records":1000,"retention_days":30,"theme":"dark","panel_opacity":94,"panel_radius":20,"enable_blur":false,"enable_animation":true,"font_size":16,"default_paste_mode":"original","auto_close_on_paste":true,"enable_sensitive_detection":true,"sensitive_auto_expire_seconds":600,"auto_start":false,"minimize_to_tray":true,"ignored_apps":[]}"#;
        let s: Settings = serde_json::from_str(json).expect("parse");
        assert!(!s.enable_ai, "upgrades must keep AI off");
        assert_eq!(s.ai_max_chars, 4000);
        assert_eq!(s.ai_min_chars, 32);
        assert!(s.ai_summary_alias && s.ai_auto_tag);
    }

    #[test]
    fn missing_json_field_skips_onboarding_for_upgrades() {
        let json = r#"{"global_shortcut":"Ctrl+Shift+V","max_records":1000,"retention_days":30,"theme":"dark","panel_opacity":94,"panel_radius":20,"enable_blur":false,"enable_animation":true,"font_size":16,"default_paste_mode":"original","auto_close_on_paste":true,"enable_sensitive_detection":true,"sensitive_auto_expire_seconds":600,"auto_start":false,"minimize_to_tray":true,"ignored_apps":[]}"#;
        let s: Settings = serde_json::from_str(json).expect("parse");
        assert!(s.onboarding_completed);
        assert_eq!(s.search_mode, "full");
    }
}
