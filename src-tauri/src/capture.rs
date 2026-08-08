//! Clipboard capture pipeline: text/image worker threads + the monitor wiring.
//! Split out of lib.rs so the app-entry file stays small.

use parking_lot::RwLock;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use tauri::Emitter;
use tracing::{info, warn};

use crate::clipboard::{
    get_foreground_window_info, CapturedImage, CapturedText, ClipboardEvent, ClipboardMonitor,
};
use crate::db::{ClipboardDb, ContentType, ImageMeta};
use crate::detect::{detect_content_type, detect_sensitive, sha256_hash, sha256_hash_bytes};
use crate::media;
use crate::panel::{is_ignored_app, list_ipc_payload};
use crate::Settings;

// ============================================================
// Capture Job Types & Workers (C-1: split text/image pipelines)
// ============================================================

/// Lightweight text capture job — processed by the fast text worker thread.
struct TextCaptureJob {
    captured: CapturedText,
    source_app: String,
    source_window: String,
    source_name: String,
}

/// Heavy image capture job — processed by the dedicated image worker thread
/// so PNG encode + thumbnail generation never blocks text captures.
struct ImageCaptureJob {
    captured: CapturedImage,
    source_app: String,
    source_window: String,
    source_name: String,
}

/// Spawn the capture pipeline (text worker + image worker + monitor) at
/// startup, first, to minimise the startup blind spot.
pub(crate) fn start_capture(
    app: &tauri::AppHandle,
    db: Arc<ClipboardDb>,
    monitor: Arc<RwLock<ClipboardMonitor>>,
    capture_paused: Arc<RwLock<bool>>,
) {
    let media_root = db.media_root().to_path_buf();

    // Text worker: detect + hash + DB insert (<5ms per job).
    let (text_tx, text_rx) = mpsc::sync_channel::<TextCaptureJob>(4);
    let db_text = db.clone();
    let app_text = app.clone();
    std::thread::spawn(move || {
        while let Ok(job) = text_rx.recv() {
            // A panic here (DB error, malformed data) must not kill capture:
            // recover, log, and keep draining the queue.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_text_job(job, &db_text, &app_text);
            }))
            .is_err()
            {
                warn!("Text capture worker recovered from panic");
            }
        }
    });

    // Image worker: RGBA → PNG encode → thumbnail → DB insert (50-300ms).
    // Capacity 2: at most 2 queued + 1 in-flight; full queue drops (poll
    // thread must not block). Pre-channel downscaling caps RGBA at ~26MB.
    let (image_tx, image_rx) = mpsc::sync_channel::<ImageCaptureJob>(2);
    let db_image = db.clone();
    let app_image = app.clone();
    let media_root_image = media_root.clone();
    std::thread::spawn(move || {
        while let Ok(job) = image_rx.recv() {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_image_job(job, &db_image, &media_root_image, &app_image);
            }))
            .is_err()
            {
                warn!("Image capture worker recovered from panic");
            }
        }
    });

    let capture_paused_thread = capture_paused;
    std::thread::spawn(move || {
        monitor.write().start(move |event| {
            if *capture_paused_thread.read() {
                return;
            }
            let (source_window, source_app, source_name) = get_foreground_window_info();
            // Dispatch to the appropriate worker: text (fast) or image (slow).
            // Non-blocking: a full queue must not stall the poll thread.
            match event {
                ClipboardEvent::Text(captured) => {
                    let job = TextCaptureJob {
                        captured,
                        source_app,
                        source_window,
                        source_name,
                    };
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
                    let job = ImageCaptureJob {
                        captured,
                        source_app,
                        source_window,
                        source_name,
                    };
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
}

/// Text worker: detect content type, hash, dedup, insert (<5ms per job).
fn process_text_job(job: TextCaptureJob, db: &ClipboardDb, app: &tauri::AppHandle) {
    let TextCaptureJob {
        captured,
        source_app,
        source_window,
        source_name,
    } = job;
    let settings = capture_settings(db);
    if is_ignored_app(&source_app, &settings.ignored_apps) {
        return;
    }

    let content_type = detect_content_type(&captured.text);
    let is_sensitive = settings.enable_sensitive_detection && detect_sensitive(&captured.text);
    // fingerprint is already sha256(text); the extra wrap keeps the stored
    // value in the historical double-hash format so the one-shot
    // `text_hash_v2` migration can re-derive identical hashes from stored
    // content alone (no html needed).
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
        &source_name,
        None,
        captured.html.as_deref(),
    ) {
        Ok((id, is_new, mut record)) => {
            if is_new && settings.features.tags && settings.enable_auto_tag {
                if let Err(e) =
                    db.apply_auto_tags(id, &captured.text, &content_type, &settings.auto_tag_rules)
                {
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
    media_root: &Path,
    app: &tauri::AppHandle,
) {
    let ImageCaptureJob {
        captured,
        source_app,
        source_window,
        source_name,
    } = job;
    let settings = capture_settings(db);
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
            // Destructure upfront — `image_meta` consumes media_path/thumb_path,
            // but the insert-error branch below still needs them for cleanup.
            let media::StoredImage {
                media_path,
                thumb_path,
                width,
                height,
                created,
            } = stored;
            let image_meta = ImageMeta {
                media_path,
                thumb_path,
                width: width as i32,
                height: height as i32,
            };
            let label = format!("[image {}x{}]", width, height);
            match db.insert_record(
                &label,
                &ContentType::Image,
                &hash,
                false,
                settings.max_records,
                settings.sensitive_auto_expire_seconds,
                &source_app,
                &source_window,
                &source_name,
                Some(&image_meta),
                None,
            ) {
                Ok((id, is_new, mut record)) => {
                    if is_new && settings.features.tags && settings.enable_auto_tag {
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
                Err(e) => {
                    warn!("Failed to insert image record: {}", e);
                    // The files were freshly written by this store call — without
                    // a row to reference them they are orphans. Only delete when
                    // `created` is true so we never touch a file another active
                    // row may share (file-level dedup hit).
                    if created {
                        media::delete_media_files(
                            media_root,
                            Some(&image_meta.media_path),
                            Some(&image_meta.thumb_path),
                        );
                    }
                }
            }
        }
        Err(e) => warn!("Failed to store clipboard image: {}", e),
    }
}

pub(crate) const CLEANUP_INTERVAL_SECS: u64 = 60;

/// Load settings for a capture worker, logging (not silently defaulting) on
/// failure. Defaults are a deliberate degrade so a transient DB read error
/// does not drop a clipboard event entirely, but the failure stays visible.
fn capture_settings(db: &ClipboardDb) -> Settings {
    match db.get_settings() {
        Ok(s) => (*s).clone(),
        Err(e) => {
            warn!("Failed to load settings for capture; using defaults: {}", e);
            Settings::default()
        }
    }
}

/// Background cleanup: expire sensitive rows + retention. Does not run on capture.
pub(crate) fn run_periodic_cleanup(db: &ClipboardDb) -> Result<Vec<i64>, String> {
    let expired = db.cleanup_expired().map_err(|e| e.to_string())?;
    if let Ok(settings) = db.get_settings() {
        db.cleanup_retention(settings.retention_days)
            .map_err(|e| e.to_string())?;
    }
    Ok(expired)
}
