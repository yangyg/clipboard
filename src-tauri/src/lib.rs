mod capture;
mod clipboard;
mod commands;
mod db;
mod detect;
mod features;
mod ffi;
mod media;
mod panel;
mod security;
mod setup;
mod tray;
mod types;
mod webdav;
mod window;

pub use features::{require_feature, FeatureFlags, FeatureId};
pub use types::{
    AppState, AutoTagRule, ClipboardRecord, RecordsPage, SearchResult, Settings, StatsData, TagInfo,
};

pub(crate) use panel::{apply_global_shortcut, show_main_panel, toggle_main_panel};

use clipboard::ClipboardMonitor;
use db::ClipboardDb;
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing::{error, info};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    info!("Clipboard starting up...");

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None::<Vec<&'static str>>,
        ))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_records,
            commands::search_records,
            commands::get_record,
            commands::open_record_media,
            commands::open_url,
            commands::capture_paste_target,
            commands::paste_record,
            commands::delete_record,
            commands::delete_records_batch,
            commands::restore_record,
            commands::restore_records_batch,
            commands::permanently_delete_record,
            commands::permanently_delete_records_batch,
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
            commands::set_window_backdrop,
            commands::get_all_tags,
            commands::create_tag,
            commands::delete_tag,
            commands::update_tag,
            commands::add_tag_to_record,
            commands::remove_tag_from_record,
            commands::set_record_tags,
        ])
        .setup(move |app| {
            setup::setup(
                app,
                db_for_setup.clone(),
                monitor_for_setup.clone(),
                capture_paused_for_setup.clone(),
            )
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    window.hide().ok();
                    api.prevent_close();
                }
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
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
