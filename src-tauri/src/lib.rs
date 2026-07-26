mod db;
mod clipboard;
mod media;
mod detect;
mod commands;
mod window;
mod tray;
mod security;
mod webdav;

use db::{ClipboardDb, ContentType, ImageMeta};
use clipboard::{CapturedImage, CapturedText, ClipboardEvent, ClipboardMonitor};
use detect::{detect_content_type, detect_sensitive, sha256_hash, sha256_hash_bytes};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use parking_lot::RwLock;
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

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
    #[serde(rename = "enable_animation")]
    pub enable_animation: bool,
    #[serde(rename = "font_size")]
    pub font_size: i32,
    #[serde(rename = "app_mode")]
    pub app_mode: String,
    #[serde(rename = "default_paste_mode")]
    pub default_paste_mode: String,
    #[serde(rename = "auto_close_on_paste")]
    pub auto_close_on_paste: bool,
    #[serde(rename = "enable_sensitive_detection")]
    pub enable_sensitive_detection: bool,
    #[serde(rename = "sensitive_auto_expire_seconds")]
    pub sensitive_auto_expire_seconds: i32,
    #[serde(rename = "data_path")]
    pub data_path: String,
    #[serde(rename = "auto_start")]
    pub auto_start: bool,
    #[serde(rename = "minimize_to_tray")]
    pub minimize_to_tray: bool,
    #[serde(rename = "ignored_apps")]
    pub ignored_apps: Vec<String>,
    /// Remembered logical size (0 = use adaptive default). Per app mode.
    #[serde(default, rename = "floating_width")]
    pub floating_width: i32,
    #[serde(default, rename = "floating_height")]
    pub floating_height: i32,
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
    #[serde(default, rename = "webdav_last_sync_at")]
    pub webdav_last_sync_at: Option<String>,
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
            enable_animation: true,
            font_size: 16,
            app_mode: "floating".to_string(),
            default_paste_mode: "original".to_string(),
            auto_close_on_paste: true,
            enable_sensitive_detection: true,
            sensitive_auto_expire_seconds: 600,
            data_path: "".to_string(),
            auto_start: false,
            minimize_to_tray: true,
            ignored_apps: vec![
                "1Password.exe".to_string(),
                "ICBCNetBank.exe".to_string(),
            ],
            floating_width: 0,
            floating_height: 0,
            window_width: 0,
            window_height: 0,
            enable_auto_tag: true,
            auto_tag_rules: default_auto_tag_rules(),
            onboarding_completed: false,
            webdav_url: String::new(),
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_remote_path: default_webdav_remote_path(),
            webdav_sync_sensitive: false,
            webdav_device_id: String::new(),
            webdav_last_sync_at: None,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub is_auto: bool,
    pub count: i64,
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

// ============================================================
// Main Application Setup
// ============================================================

fn setup_logging(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "clipvault.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    // L-1: The non-blocking writer guard must live until process exit (dropping it
    // flushes + stops the background writer thread). Use mem::forget instead of
    // Box::leak — same effect, clearer intent (no dangling &'static mut created).
    std::mem::forget(_guard);
}

pub fn run() {
    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ClipVault");
    std::fs::create_dir_all(&app_data_dir).ok();

    setup_logging(&app_data_dir);
    info!("ClipVault starting up...");

    let db_path = app_data_dir.join("clipvault.db");
    media::ensure_dirs(&app_data_dir).ok();
    let db = match ClipboardDb::new(&db_path, app_data_dir.clone()) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };
    info!("Database initialized at {:?}", db_path);

    let monitor = Arc::new(RwLock::new(ClipboardMonitor::new()));
    let capture_paused = Arc::new(RwLock::new(false));

    let app_state = AppState {
        db: db.clone(),
        monitor: monitor.clone(),
        capture_paused: capture_paused.clone(),
    };

    let db_for_setup = db.clone();
    let monitor_for_setup = monitor.clone();
    let capture_paused_for_setup = capture_paused.clone();

    tauri::Builder::default()
        // Must be registered first so a second process exits before grabbing OS resources
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            info!("Second instance detected; focusing existing window");
            crate::show_main_panel(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None::<Vec<&'static str>>))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_records,
            commands::search_records,
            commands::get_record,
            commands::open_record_media,
            commands::capture_paste_target,
            commands::paste_record,
            commands::delete_record,
            commands::delete_records_batch,
            commands::restore_record,
            commands::restore_records_batch,
            commands::permanently_delete_record,
            commands::empty_trash,
            commands::cleanup_expired,
            commands::get_trash_count,
            commands::toggle_favorite,
            commands::batch_set_favorite,
            commands::toggle_pin,
            commands::set_record_alias,
            commands::get_settings,
            commands::save_settings,
            commands::set_capture_paused,
            commands::get_tray_menu_state,
            commands::tray_menu_action,
            commands::export_data,
            commands::import_data,
            commands::import_data_from_path,
            commands::webdav_test_connection,
            commands::webdav_pull,
            commands::webdav_push,
            commands::webdav_sync,
            commands::clear_history,
            commands::get_stats,
            commands::switch_app_mode,
            commands::set_window_corner_radius,
            commands::get_all_tags,
            commands::create_tag,
            commands::delete_tag,
            commands::update_tag,
            commands::add_tag_to_record,
            commands::remove_tag_from_record,
            commands::set_record_tags,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let db = db_for_setup.clone();
            let monitor = monitor_for_setup.clone();
            let capture_paused_thread = capture_paused_for_setup.clone();

            // Sync OS autostart with persisted setting; skip if settings cannot be loaded
            match db.get_settings() {
                Ok(startup_settings) => {
                    if let Err(e) = commands::apply_autostart(&app_handle, startup_settings.auto_start) {
                        warn!("Startup autostart sync failed: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to load settings for autostart sync: {}", e);
                }
            }

            let shortcut = db
                .get_settings()
                .map(|s| s.global_shortcut.clone())
                .unwrap_or_else(|_| "Ctrl+Shift+V".into());
            if let Err(e) = apply_global_shortcut(app.handle(), &shortcut) {
                warn!("Failed to register global shortcut {}: {}", shortcut, e);
                let shortcut_label = shortcut.clone();
                app.dialog()
                    .message(format!(
                        "全局快捷键 {shortcut_label} 已被其他程序占用，无法注册。\n\n\
                         请关闭占用该快捷键的应用后重新启动剪贴板管理，或在设置中更换快捷键。\
                         若本机已有另一个剪贴板管理在运行，请先退出那个实例。"
                    ))
                    .title("剪贴板管理")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }

            // Setup system tray
            tray::build_tray(app.handle(), capture_paused_for_setup.clone())?;
            tray::start_resume_watcher(app.handle().clone());

            // Clip main window to rounded corners (avoids rectangular / black corners on Windows)
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let radius = db
                    .get_settings()
                    .map(|s| s.panel_radius)
                    .unwrap_or(20);
                if let Err(e) = window::apply_window_round_corners(&window, radius) {
                    warn!("Failed to apply window round corners: {}", e);
                }
            }

            // Clipboard monitor only enqueues; separate workers handle text (fast)
            // and image (slow: PNG encode + thumbnail) so a large image capture
            // never blocks subsequent text captures.
            let media_root = db.media_root().to_path_buf();

            // Text worker: detect + hash + DB insert (<5ms per job).
            let (text_tx, text_rx) = mpsc::sync_channel::<TextCaptureJob>(4);
            let db_text = db.clone();
            let app_text = app_handle.clone();
            std::thread::spawn(move || {
                while let Ok(job) = text_rx.recv() {
                    process_text_job(job, &db_text, &app_text);
                }
            });

            // Image worker: RGBA → PNG encode → thumbnail → DB insert (50-300ms).
            // Capacity 2: at most 2 queued + 1 in-flight; full queue drops (poll
            // thread must not block). Pre-channel downscaling caps RGBA at ~26MB.
            let (image_tx, image_rx) = mpsc::sync_channel::<ImageCaptureJob>(2);
            let db_image = db.clone();
            let app_image = app_handle.clone();
            let media_root_image = media_root.clone();
            std::thread::spawn(move || {
                while let Ok(job) = image_rx.recv() {
                    process_image_job(job, &db_image, &media_root_image, &app_image);
                }
            });

            // Periodic cleanup off the capture path — stamp only after success.
            let db_cleanup = db.clone();
            let app_cleanup = app_handle.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(CLEANUP_INTERVAL_SECS));
                    match run_periodic_cleanup(&db_cleanup) {
                        Ok(ids) if !ids.is_empty() => {
                            let _ = app_cleanup.emit("records-expired", &ids);
                        }
                        Ok(_) => {}
                        Err(e) => warn!("Periodic cleanup failed: {}", e),
                    }
                }
            });

            std::thread::spawn(move || {
                monitor.write().start(move |event| {
                    if *capture_paused_thread.read() {
                        return;
                    }
                    let (source_window, source_app) = clipboard::get_foreground_window_info();
                    // Dispatch to the appropriate worker: text (fast) or image (slow).
                    // Non-blocking: a full queue must not stall the poll thread.
                    match event {
                        ClipboardEvent::Text(captured) => {
                            let job = TextCaptureJob { captured, source_app, source_window };
                            match text_tx.try_send(job) {
                                Ok(()) => {}
                                Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                    warn!("Text capture queue full; dropping event");
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                                    warn!("Text capture worker stopped");
                                }
                            }
                        }
                        ClipboardEvent::Image(captured) => {
                            let job = ImageCaptureJob { captured, source_app, source_window };
                            match image_tx.try_send(job) {
                                Ok(()) => {}
                                Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                    warn!("Image capture queue full; dropping event");
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                                    warn!("Image capture worker stopped");
                                }
                            }
                        }
                    }
                });
            });

            info!("ClipVault setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    window.hide().ok();
                    api.prevent_close();
                }
                tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    if window.label() != "main" {
                        return;
                    }
                    let app = window.app_handle().clone();
                    let radius = app
                        .try_state::<AppState>()
                        .and_then(|s| s.db.get_settings().ok())
                        .map(|s| s.panel_radius)
                        .unwrap_or(20);
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = window::apply_window_round_corners(&w, radius);
                    }
                    // Remember user-adjusted size (debounced); skip maximized.
                    if matches!(event, tauri::WindowEvent::Resized(_)) {
                        window::schedule_persist_window_size(app);
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Extra safety net — on some platforms the event loop resumes after sleep.
            // Windows sleep/wake is primarily handled by tray::start_resume_watcher.
            if let tauri::RunEvent::Resumed = event {
                tray::recover_after_resume(app_handle);
            }
        });
}

// ============================================================
// Capture Job Types & Workers (C-1: split text/image pipelines)
// ============================================================

/// Lightweight text capture job — processed by the fast text worker thread.
struct TextCaptureJob {
    captured: CapturedText,
    source_app: String,
    source_window: String,
}

/// Heavy image capture job — processed by the dedicated image worker thread
/// so PNG encode + thumbnail generation never blocks text captures.
struct ImageCaptureJob {
    captured: CapturedImage,
    source_app: String,
    source_window: String,
}

/// Text worker: detect content type, hash, dedup, insert (<5ms per job).
fn process_text_job(
    job: TextCaptureJob,
    db: &ClipboardDb,
    app: &tauri::AppHandle,
) {
    let TextCaptureJob { captured, source_app, source_window } = job;
    let settings = db.get_settings().unwrap_or_default();
    if is_ignored_app(&source_app, &settings.ignored_apps) {
        return;
    }

    let content_type = detect_content_type(&captured.text);
    let is_sensitive =
        settings.enable_sensitive_detection && detect_sensitive(&captured.text);
    // Keep wrapping fingerprint for DB hash so existing rows still dedupe
    // (historical inserts stored sha256(fingerprint), not fingerprint itself).
    let hash = sha256_hash(&captured.fingerprint());

    match db.insert_record(
        &captured.text,
        &content_type,
        &hash,
        is_sensitive,
        settings.max_records,
        settings.sensitive_auto_expire_seconds,
        &source_app,
        &source_window,
        None,
        captured.html.as_deref(),
    ) {
        Ok((id, is_new, mut record)) => {
            if is_new && settings.enable_auto_tag {
                if let Err(e) = db.apply_auto_tags(
                    id,
                    &captured.text,
                    &content_type,
                    &settings.auto_tag_rules,
                ) {
                    warn!("Failed to apply auto tags: {}", e);
                } else if let Ok(tags) = db.get_record_tag_names(id) {
                    record.tags = tags;
                }
            }
            info!(
                "New clipboard record: id={}, type={}, formatted={}, is_new={}",
                id,
                content_type,
                captured.html.is_some(),
                is_new
            );
            app.emit("clipboard-changed", list_ipc_payload(record)).ok();
        }
        Err(e) => warn!("Failed to insert text record: {}", e),
    }
}

/// Image worker: RGBA → PNG encode → downscale → thumbnail → DB insert (50-300ms).
/// Runs on its own thread so heavy encoding never starves text captures.
fn process_image_job(
    job: ImageCaptureJob,
    db: &ClipboardDb,
    media_root: &PathBuf,
    app: &tauri::AppHandle,
) {
    let ImageCaptureJob { captured, source_app, source_window } = job;
    let settings = db.get_settings().unwrap_or_default();
    if is_ignored_app(&source_app, &settings.ignored_apps) {
        return;
    }

    let hash = if captured.hash.is_empty() {
        sha256_hash_bytes(&captured.rgba)
    } else {
        captured.hash
    };
    match media::store_clipboard_image(
        media_root,
        captured.rgba,
        captured.width,
        captured.height,
        &hash,
    ) {
        Ok(stored) => {
            let image_meta = ImageMeta {
                media_path: stored.media_path,
                thumb_path: stored.thumb_path,
                width: stored.width as i32,
                height: stored.height as i32,
            };
            let label = format!("[image {}x{}]", stored.width, stored.height);
            match db.insert_record(
                &label,
                &ContentType::Image,
                &hash,
                false,
                settings.max_records,
                settings.sensitive_auto_expire_seconds,
                &source_app,
                &source_window,
                Some(&image_meta),
                None,
            ) {
                Ok((id, is_new, mut record)) => {
                    if is_new && settings.enable_auto_tag {
                        if let Err(e) = db.apply_auto_tags(
                            id,
                            &label,
                            &ContentType::Image,
                            &settings.auto_tag_rules,
                        ) {
                            warn!("Failed to apply auto tags: {}", e);
                        } else if let Ok(tags) = db.get_record_tag_names(id) {
                            record.tags = tags;
                        }
                    }
                    info!(
                        "New clipboard record: id={}, type=image, is_new={}",
                        id, is_new
                    );
                    app.emit("clipboard-changed", list_ipc_payload(record)).ok();
                }
                Err(e) => warn!("Failed to insert image record: {}", e),
            }
        }
        Err(e) => warn!("Failed to store clipboard image: {}", e),
    }
}

const CLEANUP_INTERVAL_SECS: u64 = 60;

/// Background cleanup: expire sensitive rows + retention. Does not run on capture.
fn run_periodic_cleanup(db: &ClipboardDb) -> Result<Vec<i64>, String> {
    let expired = db.cleanup_expired().map_err(|e| e.to_string())?;
    if let Ok(settings) = db.get_settings() {
        db.cleanup_retention(settings.retention_days)
            .map_err(|e| e.to_string())?;
    }
    Ok(expired)
}

fn is_ignored_app(source_app: &str, ignored: &[String]) -> bool {
    if source_app.is_empty() || ignored.is_empty() {
        return false;
    }
    let app_lower = source_app.to_lowercase();
    ignored.iter().any(|pat| {
        let p = pat.trim().to_lowercase();
        !p.is_empty() && (app_lower == p || app_lower.ends_with(&p) || app_lower.contains(&p))
    })
}

/// Remember the previous foreground app, then show + focus the main panel.
pub(crate) fn show_main_panel(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let our = window.hwnd().ok().map(|h| h.0 as isize);
        clipboard::set_our_main_hwnd(our);
        clipboard::remember_paste_target(our);
        let _ = window.unminimize();
        let _ = window.show();
        if let Some(hwnd) = our {
            let _ = clipboard::focus_window(hwnd);
        } else {
            let _ = window.set_focus();
        }
        let _ = app.emit("toggle-panel", true);
    }
}

/// Toggle main panel. Minimized windows count as "hidden" — restore + focus
/// instead of hide (Windows keeps WS_VISIBLE while minimized).
/// Visible but not foreground → bring to front (tray / shortcut expect activate).
pub(crate) fn toggle_main_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let minimized = window.is_minimized().unwrap_or(false);
    if minimized {
        show_main_panel(app);
        return;
    }
    if window.is_visible().unwrap_or(false) {
        let in_foreground = window
            .hwnd()
            .ok()
            .map(|h| clipboard::is_foreground_hwnd(h.0 as isize))
            .unwrap_or(false)
            || window.is_focused().unwrap_or(false);
        if in_foreground {
            let _ = window.hide();
            let _ = app.emit("toggle-panel", false);
        } else {
            show_main_panel(app);
        }
    } else {
        show_main_panel(app);
    }
}

pub(crate) fn apply_global_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    let shortcut = shortcut.trim();
    if shortcut.is_empty() {
        return Err("快捷键不能为空".into());
    }
    // Unregister any previously registered shortcuts for this app instance
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister shortcuts before rebind: {}", e);
    }
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                crate::toggle_main_panel(app);
            }
        })
        .map_err(|e| e.to_string())?;
    info!("Registered global shortcut: {}", shortcut);
    Ok(())
}

/// Strip HTML and truncate content for clipboard-changed IPC (list stays light).
fn list_ipc_payload(mut r: ClipboardRecord) -> ClipboardRecord {
    r.content_html = None;
    let full_len = r
        .content_len
        .unwrap_or_else(|| r.content.chars().count() as i32);
    r.content_len = Some(full_len);
    const MAX: usize = 400;
    if (full_len as usize) > MAX {
        r.content = r.content.chars().take(MAX).collect();
    }
    r
}

#[cfg(test)]
mod settings_onboarding_tests {
    use super::Settings;

    #[test]
    fn default_settings_needs_onboarding() {
        assert!(!Settings::default().onboarding_completed);
    }

    #[test]
    fn missing_json_field_skips_onboarding_for_upgrades() {
        let json = r#"{"global_shortcut":"Ctrl+Shift+V","max_records":1000,"retention_days":30,"theme":"dark","panel_opacity":94,"panel_radius":20,"enable_blur":false,"enable_animation":true,"font_size":16,"app_mode":"floating","default_paste_mode":"original","auto_close_on_paste":true,"enable_sensitive_detection":true,"sensitive_auto_expire_seconds":600,"data_path":"","auto_start":false,"minimize_to_tray":true,"ignored_apps":[]}"#;
        let s: Settings = serde_json::from_str(json).expect("parse");
        assert!(s.onboarding_completed);
    }
}
