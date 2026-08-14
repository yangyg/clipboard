//! Clipboard monitor (arboard) + captured-text/image types.
//!
//! Windows: event-driven via `AddClipboardFormatListener` (a message-only
//! window receives `WM_CLIPBOARDUPDATE` the moment the OS clipboard changes),
//! with a 1s sequence-number watchdog as the reliability net:
//! - sleep/wake catch-up (clipboard changed while the machine was asleep),
//! - retry after transient `ClipboardOccupied` reads,
//! - fallback if `AddClipboardFormatListener` fails.
//!
//! Non-Windows builds keep the original 250ms poll loop; both paths share one
//! `handle_clipboard_tick` read/dedup routine. The bounded worker queue (set
//! up in lib.rs) absorbs the actual persistence; `image.rs` provides the cheap
//! fingerprint + downscale helpers; `paste.rs` owns the foreground-window
//! tracking this thread keeps fresh.

use super::image::{downscale_captured_rgba_if_large, image_quick_fingerprint};
use super::paste::track_last_foreign_foreground;
use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct CapturedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct CapturedText {
    pub text: String,
    /// CF_HTML / HTML clipboard fragment when present (Word, browser, etc.)
    pub html: Option<String>,
}

impl CapturedText {
    /// Fingerprint for change detection — plain text only.
    ///
    /// CF_HTML bytes are unstable identity: different apps emit different
    /// fragments for the same text, and our own paste re-write round-trips
    /// them with changed bytes. Including html here made identical text
    /// fork into duplicate records, so HTML is payload, not identity.
    pub fn fingerprint(&self) -> String {
        crate::detect::sha256_hash(&self.text)
    }
}

#[derive(Debug, Clone)]
pub enum ClipboardEvent {
    Text(CapturedText),
    Image(CapturedImage),
}

pub struct ClipboardMonitor {
    running: Arc<AtomicBool>,
    last_text_fp: Arc<parking_lot::Mutex<Option<String>>>,
    last_image_hash: Arc<parking_lot::Mutex<Option<String>>>,
    /// Skip emits until this instant (our own paste/set_clipboard must not re-capture).
    suppress_until: Arc<parking_lot::Mutex<Option<Instant>>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            last_text_fp: Arc::new(parking_lot::Mutex::new(None)),
            last_image_hash: Arc::new(parking_lot::Mutex::new(None)),
            suppress_until: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    /// Ignore clipboard changes for a short window after we write the clipboard ourselves.
    /// Paste re-writes the OS clipboard; the window stops the re-write from being treated
    /// as a fresh capture (extra reads, image re-encode). Text dedup identity is text-only
    /// (`fingerprint`), so a byte-changed CF_HTML round-trip can no longer fork a duplicate.
    pub fn suppress_self_writes(&self, duration: Duration) {
        *self.suppress_until.lock() = Some(Instant::now() + duration);
    }

    /// Mark self-written text as already captured.
    ///
    /// The suppression window only delays reads (the sequence watermark does
    /// not advance), so after it expires the poll re-reads our own paste. If
    /// the pasted record is not the latest capture, its fingerprint differs
    /// from `last_text_fp` and the re-read is emitted as a fresh capture —
    /// whose foreground source is the paste-target window, overwriting the
    /// record's original source via the re-copy dedup path. Syncing the
    /// baseline makes the poll absorb the re-read silently.
    pub fn mark_text_written(&self, text: &str) {
        *self.last_text_fp.lock() = Some(crate::detect::sha256_hash(text));
    }

    /// Mark a self-written image (RGBA path) as already captured; see
    /// [`mark_text_written`] for why the baseline must be synced.
    pub fn mark_image_written(&self, quick_fp: &str) {
        *self.last_image_hash.lock() = Some(quick_fp.to_string());
    }

    pub fn start<F>(&mut self, on_change: F)
    where
        F: Fn(ClipboardEvent) + Send + 'static,
    {
        if self.running.load(Ordering::SeqCst) {
            warn!("Clipboard monitor already running");
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let last_text_fp = self.last_text_fp.clone();
        let last_image_hash = self.last_image_hash.clone();
        let suppress_until = self.suppress_until.clone();
        let on_change: Box<dyn Fn(ClipboardEvent) + Send + 'static> = Box::new(on_change);

        std::thread::Builder::new()
            .name("clipvault-clipboard-watch".into())
            .spawn(move || {
                // Baseline fingerprint so pre-existing clipboard content is not
                // re-captured. A transient busy clipboard here just means "no
                // baseline" — the first tick handles that case normally. Runs
                // on this thread so all arboard access stays single-threaded.
                if let Ok(mut clipboard) = Clipboard::new() {
                    if let Ok(Some(captured)) = read_clipboard_text(&mut clipboard) {
                        *last_text_fp.lock() = Some(captured.fingerprint());
                    }
                }
                let last_seq = AtomicU32::new(clipboard_sequence_number());

                #[cfg(windows)]
                run_event_loop(
                    running,
                    last_text_fp,
                    last_image_hash,
                    suppress_until,
                    last_seq,
                    on_change,
                );
                #[cfg(not(windows))]
                run_poll_loop(
                    running,
                    last_text_fp,
                    last_image_hash,
                    suppress_until,
                    last_seq,
                    on_change,
                );
            })
            .expect("failed to spawn clipboard monitor thread");
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Windows event loop: a message-only window registered with
/// `AddClipboardFormatListener` receives `WM_CLIPBOARDUPDATE` on every OS
/// clipboard change, plus two timers:
/// - `TIMER_DEBOUNCE` (150ms): folds the multiple notifications one logical
///   copy emits (apps set text/HTML/bitmap formats separately) into a single
///   read, so a copy never triggers repeated `get_image()` RGBA copies.
/// - `TIMER_WATCHDOG` (1s, 250ms while a read is deferred): sequence-number
///   catch-up after sleep/resume, retry after `ClipboardOccupied`, and the
///   fallback path if the listener registration failed. This makes the
///   monitor degrade gracefully instead of silently missing captures.
#[cfg(windows)]
fn run_event_loop(
    running: Arc<AtomicBool>,
    last_text_fp: Arc<parking_lot::Mutex<Option<String>>>,
    last_image_hash: Arc<parking_lot::Mutex<Option<String>>>,
    suppress_until: Arc<parking_lot::Mutex<Option<Instant>>>,
    last_seq: AtomicU32,
    on_change: Box<dyn Fn(ClipboardEvent) + Send + 'static>,
) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::DataExchange::AddClipboardFormatListener;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
        RegisterClassW, SetTimer, TranslateMessage, CW_USEDEFAULT, MSG, WM_CLIPBOARDUPDATE,
        WM_QUIT, WM_TIMER, WNDCLASSW,
    };

    const TIMER_DEBOUNCE: usize = 1;
    const TIMER_WATCHDOG: usize = 2;
    const DEBOUNCE_MS: u32 = 150;
    const WATCHDOG_IDLE_MS: u32 = 1000;
    const WATCHDOG_BUSY_MS: u32 = 250;

    unsafe {
        let class_name: Vec<u16> = "ClipVaultClipboardWatch\0".encode_utf16().collect();
        let hinstance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        // Returns 0 when the class already exists — harmless on re-registration.
        RegisterClassW(&wc);

        const HWND_MESSAGE: HWND = -3isize as HWND;
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            warn!("Failed to create clipboard-watch message window");
            running.store(false, Ordering::SeqCst);
            return;
        }

        if AddClipboardFormatListener(hwnd) == 0 {
            // Rare failure: the 1s watchdog below still captures every change
            // (up to 1s latency) — the monitor degrades, never goes silent.
            warn!("AddClipboardFormatListener failed; falling back to timer-driven polling");
        }
        SetTimer(hwnd, TIMER_WATCHDOG, WATCHDOG_IDLE_MS, None);

        let mut clipboard_slot: Option<Clipboard> = Clipboard::new().ok();
        let mut busy_logged = false;
        info!("Clipboard monitor started (event-driven + 1s watchdog)");

        let mut msg = std::mem::zeroed::<MSG>();
        while running.load(Ordering::SeqCst) {
            // Retrieves messages for this thread (the message-only window
            // above); WM_TIMER wakes the loop at least once per second so a
            // stop() is honoured within ~1s even with no clipboard activity.
            let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if ret == -1 || ret == 0 {
                break;
            }
            match msg.message {
                WM_CLIPBOARDUPDATE => {
                    // Restarting the same timer id extends the debounce window,
                    // coalescing burst notifications from one logical copy.
                    SetTimer(hwnd, TIMER_DEBOUNCE, DEBOUNCE_MS, None);
                }
                WM_TIMER => {
                    let timer_id = msg.wParam;
                    if timer_id == TIMER_DEBOUNCE || timer_id == TIMER_WATCHDOG {
                        if timer_id == TIMER_DEBOUNCE {
                            KillTimer(hwnd, TIMER_DEBOUNCE);
                        }
                        // Refresh the paste destination while the user works in
                        // other apps (foreground changes are not clipboard
                        // events, so this rides the watchdog cadence).
                        track_last_foreign_foreground();
                        let busy = handle_clipboard_tick(
                            &mut clipboard_slot,
                            &last_seq,
                            &last_text_fp,
                            &last_image_hash,
                            &suppress_until,
                            &mut busy_logged,
                            &*on_change,
                        );
                        SetTimer(
                            hwnd,
                            TIMER_WATCHDOG,
                            if busy {
                                WATCHDOG_BUSY_MS
                            } else {
                                WATCHDOG_IDLE_MS
                            },
                            None,
                        );
                    }
                }
                WM_QUIT => break,
                _ => {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        DestroyWindow(hwnd);
        debug!("Clipboard monitor stopped");
    }
}

/// Non-Windows fallback: the original fixed-cadence poll loop, driven by the
/// same `handle_clipboard_tick` routine.
#[cfg(not(windows))]
fn run_poll_loop(
    running: Arc<AtomicBool>,
    last_text_fp: Arc<parking_lot::Mutex<Option<String>>>,
    last_image_hash: Arc<parking_lot::Mutex<Option<String>>>,
    suppress_until: Arc<parking_lot::Mutex<Option<Instant>>>,
    last_seq: AtomicU32,
    on_change: Box<dyn Fn(ClipboardEvent) + Send + 'static>,
) {
    let poll_interval = Duration::from_millis(250);
    let mut clipboard_slot: Option<Clipboard> = Clipboard::new().ok();
    info!(
        "Clipboard monitor started (poll every {}ms)",
        poll_interval.as_millis()
    );
    let mut busy_logged = false;

    while running.load(Ordering::SeqCst) {
        track_last_foreign_foreground();
        handle_clipboard_tick(
            &mut clipboard_slot,
            &last_seq,
            &last_text_fp,
            &last_image_hash,
            &suppress_until,
            &mut busy_logged,
            &*on_change,
        );
        thread::sleep(poll_interval);
    }

    debug!("Clipboard monitor stopped");
}

/// One read/dedup pass shared by the event loop and the poll fallback.
///
/// Returns `true` when the clipboard was busy and this sequence transition
/// must be retried (the caller should re-arm quickly); `false` when nothing
/// was read or the pass completed. The sequence watermark (`last_seq`) is
/// committed only after every read in the pass succeeds — a transient
/// `ClipboardOccupied` never consumes a transition. The paste-suppression
/// window skips all reads and leaves the watermark untouched, so the
/// post-window re-read absorbs our own paste via hash dedup.
fn handle_clipboard_tick(
    clipboard_slot: &mut Option<Clipboard>,
    last_seq: &AtomicU32,
    last_text_fp: &parking_lot::Mutex<Option<String>>,
    last_image_hash: &parking_lot::Mutex<Option<String>>,
    suppress_until: &parking_lot::Mutex<Option<Instant>>,
    busy_logged: &mut bool,
    on_change: &dyn Fn(ClipboardEvent),
) -> bool {
    // Paste-suppression window: the clipboard holds our own paste. Skip ALL
    // reads AND the sequence watermark — if a real copy lands in this window,
    // the first pass after it expires sees a fresh sequence and captures it
    // instead of skipping it forever.
    if is_capture_suppressed(suppress_until) {
        return false;
    }

    let seq = clipboard_sequence_number();
    // Sequence unchanged → skip all clipboard reads (esp. get_image RGBA copy).
    if seq != 0 && seq == last_seq.load(Ordering::Relaxed) {
        return false;
    }
    // Do NOT advance `last_seq` yet — see the function doc.

    if clipboard_slot.is_none() {
        *clipboard_slot = Clipboard::new().ok();
    }
    let Some(clipboard) = clipboard_slot.as_mut() else {
        return false;
    };

    let text = match read_clipboard_text(clipboard) {
        Ok(text) => text,
        Err(e) => {
            if !*busy_logged {
                warn!("Clipboard busy, deferring capture: {e}");
                *busy_logged = true;
            }
            return true;
        }
    };
    // The sequence watermark is committed only after ALL clipboard reads for
    // this pass succeed. If any read hits ClipboardOccupied we leave the
    // watermark untouched so the next pass retries this same transition.

    // Text was read first (above). Skip get_image() (full RGBA copy) when:
    // - meaningful share text wins over a co-existing thumb, or
    // - the clipboard has no bitmap/DIB formats at all.
    let prefer_text = text
        .as_ref()
        .map(|t| is_meaningful_share_text(&t.text))
        .unwrap_or(false);

    if prefer_text {
        if let Some(captured) = text {
            maybe_emit_text(last_text_fp, captured, on_change);
        }
        *busy_logged = false;
        last_seq.store(seq, Ordering::Relaxed);
        return false;
    }

    if clipboard_has_bitmap_format() {
        // Windows often keeps BOTH a bitmap and text:
        // - Screenshots: image + empty/stub text → keep image
        // - Browser "Copy image": image + URL-only text → keep image
        let image = match clipboard.get_image() {
            Err(e @ arboard::Error::ClipboardOccupied) => {
                if !*busy_logged {
                    warn!("Clipboard busy during image read, deferring: {e}");
                    *busy_logged = true;
                }
                return true;
            }
            Err(_) => None,
            Ok(img) => Some(img),
        };
        if let Some(img) = image {
            // Cheap fingerprint first — avoid full SHA-256 when bitmap unchanged
            let quick = image_quick_fingerprint(&img);
            let unchanged = {
                let last = last_image_hash.lock();
                matches!(&*last, Some(prev) if prev == &quick)
            };

            if unchanged {
                // Stale bitmap + new *meaningful* text (Windows often keeps
                // both). URL-only accompaniment of "Copy image" must not spawn
                // a duplicate link record on every re-copy of the same bitmap.
                if let Some(captured) = text {
                    if is_meaningful_share_text(&captured.text) {
                        maybe_emit_text(last_text_fp, captured, on_change);
                    }
                }
            } else {
                let width = img.width as u32;
                let height = img.height as u32;
                // Prefer moving owned buffer; only copy when Cow is borrowed
                let raw = match img.bytes {
                    std::borrow::Cow::Owned(v) => v,
                    std::borrow::Cow::Borrowed(b) => b.to_vec(),
                };
                // SHA-256 of full RGBA is done on the capture worker — the
                // monitor only needs the cheap quick fingerprint for dedup.
                *last_image_hash.lock() = Some(quick);
                // Cap very large bitmaps BEFORE they enter the bounded channel:
                // raw RGBA at 8K ≈ 660MB. We only need a 2560px-max edge for
                // preview + paste; store_clipboard_image() also targets MAX_EDGE.
                let (rgba, width, height) = downscale_captured_rgba_if_large(raw, width, height);
                debug!("Clipboard changed (image): {}x{}", width, height);
                on_change(ClipboardEvent::Image(CapturedImage {
                    rgba,
                    width,
                    height,
                    hash: String::new(),
                }));
            }
        } else if let Some(captured) = text {
            maybe_emit_text(last_text_fp, captured, on_change);
        }
    } else if let Some(captured) = text {
        maybe_emit_text(last_text_fp, captured, on_change);
    }

    // All reads for this pass succeeded — commit the watermark.
    *busy_logged = false;
    last_seq.store(seq, Ordering::Relaxed);
    false
}

fn is_capture_suppressed(suppress_until: &parking_lot::Mutex<Option<Instant>>) -> bool {
    let mut guard = suppress_until.lock();
    match *guard {
        Some(until) if Instant::now() < until => true,
        Some(_) => {
            *guard = None;
            false
        }
        None => false,
    }
}

/// OS clipboard generation counter. Unchanged ⇒ skip poll reads.
fn clipboard_sequence_number() -> u32 {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetClipboardSequenceNumber() -> u32;
        }
        unsafe { GetClipboardSequenceNumber() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// True when the clipboard advertises a bitmap/DIB format (cheap; no pixel copy).
fn clipboard_has_bitmap_format() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::DataExchange::IsClipboardFormatAvailable;
        // CF_BITMAP=2, CF_DIB=8, CF_DIBV5=17 — any one means get_image() may succeed.
        const CF_BITMAP: u32 = 2;
        const CF_DIB: u32 = 8;
        const CF_DIBV5: u32 = 17;
        unsafe {
            IsClipboardFormatAvailable(CF_BITMAP) != 0
                || IsClipboardFormatAvailable(CF_DIB) != 0
                || IsClipboardFormatAvailable(CF_DIBV5) != 0
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Read plain text (+ optional HTML) from the clipboard.
///
/// Returns `Err` only for a *transient* failure — the clipboard is held by
/// another process (`ClipboardOccupied`) — so the caller can keep the sequence
/// watermark and retry on the next pass. A clipboard that opened fine but holds
/// no usable text (image-only, empty, …) is `Ok(None)`, not an error.
fn read_clipboard_text(clipboard: &mut Clipboard) -> Result<Option<CapturedText>, arboard::Error> {
    match clipboard.get_text() {
        Ok(text) => {
            let html = clipboard
                .get()
                .html()
                .ok()
                .map(|h| h.trim().to_string())
                .filter(|h| !h.is_empty());
            Ok(Some(CapturedText { text, html }))
        }
        // Transient: another process holds the clipboard open (arboard retries
        // ~5×5ms internally before giving up). Propagate so the poll loop defers
        // this sequence transition instead of dropping it.
        Err(e @ arboard::Error::ClipboardOccupied) => Err(e),
        // ContentNotAvailable / conversion errors: accessible but no text.
        Err(_) => Ok(None),
    }
}

/// Prefer text over a co-existing bitmap only for real share/snippets.
///
/// Safe for image copies:
/// - Screenshots: usually no / empty text → keep image
/// - Browser "Copy image": text is often just the image URL → keep image
/// - Douyin/WeChat shares: long caption (+ embedded link) → prefer text
fn is_meaningful_share_text(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    // URL-only (or nearly) accompaniment → do not override the bitmap
    if is_primarily_url(t) {
        return false;
    }
    t.chars().count() >= 16
}

/// True when the payload is essentially one URL with negligible other text.
fn is_primarily_url(t: &str) -> bool {
    let lower = t.to_lowercase();
    let start = crate::security::LINK_PREFIXES
        .iter()
        .filter_map(|p| lower.find(p))
        .min();
    let Some(start) = start else {
        return false;
    };
    // NOTE: slice `lower`, not the original `t`. `to_lowercase()` can change the
    // byte length of a string (e.g. `ẞ`→`ss`, `İ`→`i̇`), so a byte offset computed
    // against `lower` may not be a char boundary in `t`, and slicing `t` at it
    // would panic, killing the capture monitor thread. Casing is irrelevant to
    // the whitespace-strip / char-count decisions made here.
    let before = lower[..start].trim();
    let from_url = &lower[start..];
    let url_len = from_url
        .find(|c: char| c.is_whitespace())
        .unwrap_or(from_url.len());
    let after = from_url[url_len..].trim();
    // Allow tiny stubs around the URL (filename, punctuation), not a caption.
    before.chars().count() <= 8 && after.chars().count() <= 8
}

fn maybe_emit_text(
    last_text_fp: &parking_lot::Mutex<Option<String>>,
    captured: CapturedText,
    on_change: &dyn Fn(ClipboardEvent),
) {
    let fp = captured.fingerprint();
    let should_notify = {
        let last = last_text_fp.lock();
        match &*last {
            Some(prev) if prev == &fp => false,
            _ => !captured.text.trim().is_empty(),
        }
    };
    if should_notify {
        *last_text_fp.lock() = Some(fp);
        debug!(
            "Clipboard changed (text): {} chars, html={}",
            captured.text.len(),
            captured.html.is_some()
        );
        on_change(ClipboardEvent::Text(captured));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // --- CapturedText::fingerprint ---

    #[test]
    fn fingerprint_is_deterministic() {
        let ct = CapturedText {
            text: "hello".into(),
            html: Some("<b>hello</b>".into()),
        };
        assert_eq!(ct.fingerprint(), ct.fingerprint());
    }

    #[test]
    fn fingerprint_changes_with_text() {
        let a = CapturedText {
            text: "hello".into(),
            html: None,
        };
        let b = CapturedText {
            text: "world".into(),
            html: None,
        };
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn fingerprint_ignores_html_variant() {
        // Identity is the plain text: CF_HTML bytes differ across sources and
        // paste round-trips, so they must not fork the dedup identity.
        let a = CapturedText {
            text: "hello".into(),
            html: None,
        };
        let b = CapturedText {
            text: "hello".into(),
            html: Some("<p>hi</p>".into()),
        };
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    // --- is_primarily_url / is_meaningful_share_text ---

    #[test]
    fn url_only_is_primarily_url() {
        assert!(is_primarily_url("https://example.com/path"));
        assert!(is_primarily_url("see https://example.com"));
        assert!(is_primarily_url(
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"
        ));
        assert!(is_primarily_url(
            "ed2k://|file|name.iso|123|ABCDEF0123456789ABCDEF0123456789|/"
        ));
    }

    #[test]
    fn url_with_caption_is_not_primarily_url() {
        assert!(!is_primarily_url(
            "Check out this really long descriptive caption https://example.com"
        ));
        assert!(!is_primarily_url("no link here at all"));
    }

    #[test]
    fn share_text_needs_length_and_no_dominant_url() {
        assert!(is_meaningful_share_text("this is a meaningful caption"));
        assert!(!is_meaningful_share_text("   "));
        assert!(!is_meaningful_share_text("short"));
        assert!(!is_meaningful_share_text(
            "https://example.com/some/long/path"
        ));
    }

    // --- is_capture_suppressed ---

    #[test]
    fn suppress_active_within_window() {
        let suppress = parking_lot::Mutex::new(Some(Instant::now() + Duration::from_secs(5)));
        assert!(is_capture_suppressed(&suppress));
    }

    #[test]
    fn suppress_expired_clears_value() {
        let suppress = parking_lot::Mutex::new(Some(Instant::now() - Duration::from_secs(1)));
        assert!(!is_capture_suppressed(&suppress));
        // After expiry the slot should be cleared to None
        assert!(suppress.lock().is_none());
    }

    #[test]
    fn suppress_none_is_not_suppressed() {
        let suppress = parking_lot::Mutex::new(None);
        assert!(!is_capture_suppressed(&suppress));
    }

    // --- mark_text_written / mark_image_written ---

    #[test]
    fn mark_text_written_syncs_last_text_fp() {
        let monitor = ClipboardMonitor::new();
        monitor.mark_text_written("hello");
        let captured = CapturedText {
            text: "hello".into(),
            html: None,
        };
        // Baseline must equal the fingerprint the poll loop computes, so the
        // post-suppression re-read of our own paste is absorbed (no emit).
        assert_eq!(*monitor.last_text_fp.lock(), Some(captured.fingerprint()));
    }

    #[test]
    fn mark_image_written_syncs_quick_fp() {
        let monitor = ClipboardMonitor::new();
        monitor.mark_image_written("0123456789abcdef");
        assert_eq!(
            *monitor.last_image_hash.lock(),
            Some("0123456789abcdef".to_string())
        );
    }
}
