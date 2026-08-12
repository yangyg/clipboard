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
use crate::{tray, window, Settings};

/// Ensure the device identity (sync id + display name) exists before any
/// capture can stamp records with it. Runs before the capture pipeline starts;
/// failures are logged and degrade to empty stamps, never fatal at startup.
fn ensure_device_identity(db: &ClipboardDb) {
    let Ok(current) = db.get_settings().map(|s| (*s).clone()) else {
        error!("Failed to load settings while ensuring device identity");
        return;
    };
    let mut next = current;
    let mut changed = false;
    if next.webdav_device_id.trim().is_empty() {
        next.webdav_device_id = uuid::Uuid::new_v4().to_string();
        changed = true;
    }
    if next.webdav_device_name.trim().is_empty() {
        next.webdav_device_name =
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "My Device".to_string());
        changed = true;
    }
    if changed {
        if let Err(e) = db.save_settings(&next) {
            error!("Failed to persist device identity: {}", e);
        } else {
            info!("Device identity ensured (id={})", next.webdav_device_id);
        }
    }
}

pub(crate) fn setup(
    app: &mut App,
    db: Arc<ClipboardDb>,
    monitor: Arc<RwLock<ClipboardMonitor>>,
    capture_paused: Arc<RwLock<bool>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // Device identity must exist before captures start so every new record can
    // be stamped with its origin.
    ensure_device_identity(&db);

    // One settings snapshot for the whole startup path — `get_settings` is
    // cached (Arc) after the first read, but reading it once here keeps the
    // non-critical setup branches (autostart / shortcut / window chrome / blur)
    // from re-entering the settings cache + DPAPI decrypt path sequentially.
    let startup_settings = match db.get_settings() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to load settings for startup setup: {}", e);
            Arc::new(Settings::default())
        }
    };

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

    // Sync OS autostart with persisted setting; failures are logged, never fatal.
    if let Err(e) = commands::apply_autostart(&app_handle, startup_settings.auto_start) {
        warn!("Startup autostart sync failed: {}", e);
    }

    let shortcut = startup_settings.global_shortcut.clone();
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
    // and apply single-window-mode chrome (always-on-top flag + remembered size).
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_shadow(false);
        let _ = window.set_always_on_top(startup_settings.always_on_top);
        let (min_w, min_h, _, _) = window::mode_size_bounds();
        let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
            min_w, min_h,
        ))));
        let (w, h) = window::resolve_panel_size(&window, &startup_settings);
        window::SIZE_SAVE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
        if let Err(e) = window::apply_window_round_corners(&window, startup_settings.panel_radius) {
            warn!("Failed to apply window round corners: {}", e);
        }
    }

    // Apply native frosted-glass backdrop from persisted 毛玻璃 setting.
    let blur_enabled = startup_settings.enable_blur;
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
