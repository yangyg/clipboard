//! One-time app setup, extracted from `lib.rs::run` so the entry file stays
//! small: capture pipeline, autostart sync, global shortcut, tray, native
//! theme watcher, window chrome, and the periodic cleanup thread.

use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{App, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tracing::{error, info, warn};

use crate::ai;
use crate::capture::{run_periodic_cleanup, start_capture, CLEANUP_INTERVAL_SECS};
use crate::clipboard::ClipboardMonitor;
use crate::commands;
use crate::db::ClipboardDb;
use crate::panel::apply_global_shortcut;
use crate::{tray, window};

pub(crate) fn setup(
    app: &mut App,
    db: Arc<ClipboardDb>,
    monitor: Arc<RwLock<ClipboardMonitor>>,
    capture_paused: Arc<RwLock<bool>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // ── AI enrichment worker (off the capture hot path) ──
    let ai_tx = ai::start_ai_worker(app_handle.clone(), db.clone());

    // ── Capture pipeline (start first to minimise the startup blind spot) ──
    start_capture(
        &app_handle,
        db.clone(),
        monitor.clone(),
        capture_paused.clone(),
        ai_tx,
    );

    // ── Non-critical setup (autostart, shortcut, tray, theme, window) ──

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
    tray::build_tray(app.handle(), capture_paused.clone())?;
    tray::start_resume_watcher(app.handle().clone());
    // Fix CSS cursor: pointer on the transparent WebView2 popup (WM_SETCURSOR reset).
    if let Some(tray_win) = app.get_webview_window("tray-menu") {
        tray::hook_tray_menu_cursor(&tray_win);
    }

    // Clip main window to rounded corners (avoids rectangular / black corners on Windows)
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_shadow(false);
        let radius = db.get_settings().map(|s| s.panel_radius).unwrap_or(20);
        if let Err(e) = window::apply_window_round_corners(&window, radius) {
            warn!("Failed to apply window round corners: {}", e);
        }
    }

    // Apply native frosted-glass backdrop from persisted 毛玻璃 setting.
    let blur_enabled = db.get_settings().map(|s| s.enable_blur).unwrap_or(false);
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window::apply_window_backdrop(&window, blur_enabled) {
            warn!("Failed to apply window backdrop: {}", e);
        }
    }

    // Periodic cleanup off the capture path — stamp only after success.
    let db_cleanup = db.clone();
    let app_cleanup = app_handle.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(CLEANUP_INTERVAL_SECS));
        match run_periodic_cleanup(&db_cleanup) {
            Ok(ids) if !ids.is_empty() => {
                let _ = app_cleanup.emit("records-expired", &ids);
            }
            Ok(_) => {}
            Err(e) => warn!("Periodic cleanup failed: {}", e),
        }
    });

    info!("Clipboard setup complete");
    Ok(())
}
