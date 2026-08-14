//! Startup import of the OS clipboard history (Windows 11 "Win+V").
//!
//! The OS clipboard history is only reachable over WinRT
//! (`Windows.ApplicationModel.DataTransfer.Clipboard`), and only while the app
//! is the foreground window — otherwise the call fails with `AccessDenied`.
//! The trigger is therefore the main window gaining focus (`WindowEvent::Focused`),
//! with `panel::show_main_panel` as a fallback, and the import itself runs on a
//! dedicated background thread.
//!
//! Contract:
//! - Gated by `settings.import_system_history_on_start` (default **off**).
//! - One import per session state; `AccessDenied` / transient errors leave the
//!   "done" flag clear so a later focus callback retries.
//! - Idempotent: each item is pre-checked with `record_hash_exists` and skipped
//!   when already active — re-running never bumps `updated_at` nor wipes
//!   `source_*` the way `insert_record`'s dedup-update path would.
//! - Text + image items only (HTML is not decoded). Text goes through the normal
//!   detect / sensitive / auto-tag pipeline; images reuse the ≤2560 capture +
//!   `media::store_clipboard_image` path. Source is empty → 「系统剪贴板」.
//! - All failures degrade to logs; a successful run emits one
//!   `clipboard-history-imported` event with the inserted count.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, info, warn};

use crate::db::{ContentType, ImageMeta};
use crate::media::StoredImage;
use crate::{detect, ClipboardDb, Settings};
use windows::ApplicationModel::DataTransfer::DataPackageView;

/// True once a terminal state was reached for this session: feature was off on
/// the first check, system history unavailable, or a completed run. Left clear
/// on `AccessDenied` so a later focus callback can retry.
static IMPORT_DONE: AtomicBool = AtomicBool::new(false);
/// Guards against overlapping runs (e.g. focus callback + show_main_panel).
static IMPORT_RUNNING: AtomicBool = AtomicBool::new(false);
/// Inserted count of the most recent successful run, kept for the frontend to
/// catch up on if it missed the event (the import is triggered by the first
/// `Focused` event, which can fire before the webview registered its `listen`).
/// Read-and-cleared once by `take_pending_import`.
static LAST_IMPORT_INSERTED: AtomicUsize = AtomicUsize::new(0);

/// Frontend catch-up: read the last successful run's inserted count, clearing
/// it so a single import is only ever reported once.
pub fn take_pending_import() -> usize {
    LAST_IMPORT_INSERTED.swap(0, Ordering::AcqRel)
}

/// Entry point called from window-focus / panel-show. Runs the (blocking) WinRT
/// import on a background thread so the UI thread never stalls.
pub fn maybe_start_once(app: &AppHandle) {
    if IMPORT_DONE.load(Ordering::Acquire) {
        return;
    }
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let settings = match state.db.get_settings() {
        Ok(s) => s,
        Err(e) => {
            warn!("Skipping history import — settings unavailable: {e}");
            return;
        }
    };
    if !settings.import_system_history_on_start {
        // Feature off — terminal for this session. Re-latching it runs on the
        // next app start, matching the "on startup" wording of the setting.
        IMPORT_DONE.store(true, Ordering::Release);
        return;
    }
    if IMPORT_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }

    let db = state.db.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let outcome = import_windows_history_sta(&db, &app);
        if outcome != ImportOutcome::RetryLater {
            IMPORT_DONE.store(true, Ordering::Release);
        }
        IMPORT_RUNNING.store(false, Ordering::Release);
    });
}

/// The `Clipboard` WinRT class is registered single-threaded; the background
/// thread must be an STA, otherwise activation fails with
/// `0x8000001D` ("cannot activate a single-threaded class from MTA"). The
/// default `std::thread` here is MTA because windows-core auto-inits with
/// `CoIncrementMTAUsage`, so we explicitly enter an apartment-threaded COM
/// apartment for the duration of the WinRT enumeration.
fn import_windows_history_sta(db: &ClipboardDb, app: &AppHandle) -> ImportOutcome {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if !hr.is_ok() {
        warn!("CoInitializeEx(STA) failed, skipping history import: {hr}");
        return ImportOutcome::Unavailable;
    }
    let outcome = import_windows_history(db, app);
    unsafe { CoUninitialize() };
    outcome
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportOutcome {
    Done,
    Unavailable,
    RetryLater,
}

fn import_windows_history(db: &ClipboardDb, app: &AppHandle) -> ImportOutcome {
    use windows::core::HSTRING;
    use windows::ApplicationModel::DataTransfer::{Clipboard, ClipboardHistoryItemsResultStatus};

    /// Cheap format-presence probe (format ids are "Text" / "Bitmap").
    fn contains(view: &DataPackageView, format: &str) -> bool {
        view.Contains(&HSTRING::from(format)).unwrap_or(false)
    }

    let history_enabled = match Clipboard::IsHistoryEnabled() {
        Ok(v) => v,
        Err(e) => {
            warn!("Clipboard history API unavailable (pre-1809 / no WinRT): {e}");
            return ImportOutcome::Unavailable;
        }
    };
    if !history_enabled {
        debug!("OS clipboard history is disabled; nothing to import.");
        return ImportOutcome::Unavailable;
    }

    let result = match Clipboard::GetHistoryItemsAsync().and_then(|op| op.get()) {
        Ok(r) => r,
        Err(e) => {
            warn!("GetHistoryItemsAsync failed: {e}");
            return ImportOutcome::RetryLater;
        }
    };
    match result.Status() {
        Ok(ClipboardHistoryItemsResultStatus::Success) => {}
        Ok(ClipboardHistoryItemsResultStatus::AccessDenied) => {
            debug!("Clipboard history access denied (not foreground) — retry next focus.");
            return ImportOutcome::RetryLater;
        }
        // ClipboardHistoryDisabled / unexpected status → nothing importable.
        Ok(_) => return ImportOutcome::Unavailable,
        Err(_) => return ImportOutcome::RetryLater,
    }

    let settings = match db.get_settings() {
        Ok(s) => s,
        Err(e) => {
            warn!("could not load settings for history import: {e}");
            return ImportOutcome::Unavailable;
        }
    };
    let media_root = db.media_root().to_path_buf();

    let items = match result.Items() {
        Ok(v) => v,
        Err(e) => {
            warn!("Clipboard history items unavailable: {e}");
            return ImportOutcome::Unavailable;
        }
    };

    let mut imported = 0usize;
    for item in items {
        let Ok(content) = item.Content() else {
            continue;
        };

        if contains(&content, "Text") {
            match content.GetTextAsync().and_then(|op| op.get()) {
                Ok(text) => {
                    let text = text.to_string();
                    if !text.trim().is_empty() && import_text(db, &text, &settings) {
                        imported += 1;
                    }
                }
                Err(e) => debug!("history text item unreadable: {e}"),
            }
        }

        if contains(&content, "Bitmap") {
            match import_image(db, &content, &media_root, &settings) {
                Ok(()) => imported += 1,
                Err(e) => debug!("history bitmap item skipped: {e}"),
            }
        }
    }

    if imported > 0 {
        info!("Imported {imported} record(s) from the OS clipboard history.");
    }
    LAST_IMPORT_INSERTED.store(imported, Ordering::Release);
    let _ = app.emit(
        "clipboard-history-imported",
        serde_json::json!({ "inserted": imported }),
    );
    ImportOutcome::Done
}

/// Insert a history text item unless it is already active. Returns true when a
/// new row was created. Mirrors `capture.rs::process_text_job` (sensitive +
/// auto-tag + capacity rules) but skips the dedup-update path.
fn import_text(db: &ClipboardDb, text: &str, s: &Arc<Settings>) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    // Mirror the live-capture cap: oversized history items are skipped so an
    // accidental huge copy does not bloat the DB / FTS index.
    let cap = s.max_text_bytes.max(0) as usize;
    if crate::detect::exceeds_text_byte_cap(text.len(), 0, s.max_text_bytes) {
        info!(
            "Skipping oversized history text item: {} bytes (cap {} bytes)",
            text.len(),
            cap
        );
        return false;
    }
    let content_type = detect::detect_content_type(text);
    let is_sensitive = s.enable_sensitive_detection && detect::detect_sensitive(text);
    // Historical double-hash text format (see records_write text_hash_v2).
    let hash = detect::sha256_hash(&detect::sha256_hash(text));

    let skip = match db.record_hash_exists(&hash) {
        Ok(v) => v,
        Err(e) => {
            warn!("record_hash_exists failed: {e}");
            return false;
        }
    };
    if skip {
        return false;
    }

    match db.insert_record(
        text,
        &content_type,
        &hash,
        is_sensitive,
        s.max_records,
        s.sensitive_auto_expire_seconds,
        "",
        "",
        "",
        None,
        None,
    ) {
        Ok((_id, is_new, _)) => is_new,
        Err(e) => {
            warn!("Failed to insert history text record: {e}");
            false
        }
    }
}

/// Decode a history bitmap → RGBA, reuse the live-capture store path, insert
/// `[image WxH]` record. Errors are per-item skips, so a corrupt bitmap never
/// aborts the rest of the import.
fn import_image(
    db: &ClipboardDb,
    content: &DataPackageView,
    media_root: &std::path::Path,
    s: &Arc<Settings>,
) -> Result<(), String> {
    let stream_ref = content
        .GetBitmapAsync()
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let bytes = read_stream_bytes(&stream_ref).map_err(|e| e.to_string())?;

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| format!("decode: {e}"))?;
    let rgba_img = dyn_img.to_rgba8();
    let (width, height) = (rgba_img.width(), rgba_img.height());
    if width == 0 || height == 0 {
        return Err("zero-sized bitmap".into());
    }

    // Mirror the monitor: downscale to ≤2560 before hashing so the hash matches
    // what a live capture of the same image would produce (→ cross-dedup).
    let (rgba, width, height) =
        crate::media::downscale_rgba(rgba_img.into_raw(), width, height, crate::media::MAX_EDGE);
    if rgba.is_empty() {
        return Err("downscale produced empty buffer".into());
    }
    let hash = detect::sha256_hash_bytes(&rgba);
    if db.record_hash_exists(&hash).unwrap_or(false) {
        return Ok(());
    }

    let StoredImage {
        media_path,
        thumb_path,
        width: w,
        height: h,
        created,
    } = crate::media::store_clipboard_image(media_root, rgba, width, height, &hash)?;
    let label = format!("[image {}x{}]", w, h);
    let image_meta = ImageMeta {
        media_path,
        thumb_path,
        width: w as i32,
        height: h as i32,
    };
    match db.insert_record(
        &label,
        &ContentType::Image,
        &hash,
        false,
        s.max_records,
        s.sensitive_auto_expire_seconds,
        "",
        "",
        "",
        Some(&image_meta),
        None,
    ) {
        Ok((_id, is_new, _)) => {
            if !is_new && created {
                // DB-level dedup hit — the files we just wrote are orphans.
                crate::media::delete_media_files(
                    media_root,
                    Some(&image_meta.media_path),
                    Some(&image_meta.thumb_path),
                );
            }
            Ok(())
        }
        Err(e) => {
            if created {
                crate::media::delete_media_files(
                    media_root,
                    Some(&image_meta.media_path),
                    Some(&image_meta.thumb_path),
                );
            }
            Err(format!("insert: {e}"))
        }
    }
}

/// Read the whole stream exposed by a bitmap history item into a byte buffer
/// (the `image` crate then decodes PNG/JPEG/etc.).
fn read_stream_bytes(
    stream_ref: &windows::Storage::Streams::RandomAccessStreamReference,
) -> Result<Vec<u8>, String> {
    use windows::Storage::Streams::{DataReader, IInputStream};

    let stream = stream_ref
        .OpenReadAsync()
        .and_then(|op| op.get())
        .map_err(|e| format!("open stream: {e}"))?;
    let input: IInputStream = stream.GetInputStreamAt(0).map_err(|e| e.to_string())?;
    let reader = DataReader::CreateDataReader(&input).map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    const CHUNK: u32 = 64 * 1024;
    loop {
        let loaded = reader
            .LoadAsync(CHUNK)
            .and_then(|op| op.get())
            .map_err(|e| e.to_string())?;
        if loaded == 0 {
            break;
        }
        let mut buf = vec![0u8; loaded as usize];
        // ReadBytes consumes exactly `buf.len()` available bytes; sized to `loaded`.
        reader.ReadBytes(&mut buf).map_err(|e| e.to_string())?;
        out.extend_from_slice(&buf);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_equality_smoke() {
        assert_eq!(ImportOutcome::Done, ImportOutcome::Done);
        assert_ne!(ImportOutcome::Done, ImportOutcome::Unavailable);
    }
}
