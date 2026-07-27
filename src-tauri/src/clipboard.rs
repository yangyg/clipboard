use arboard::{Clipboard, ImageData};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use image::{imageops::FilterType, RgbaImage};

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
    /// Fingerprint for change detection (plain + html) — hash only, no huge string retention.
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.text.as_bytes());
        if let Some(h) = &self.html {
            hasher.update(h.as_bytes());
        }
        hex::encode(hasher.finalize())
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
    /// Paste re-writes OS clipboard; CF_HTML round-trips often change the HTML bytes, so the
    /// text+html fingerprint no longer matches the DB hash and would insert a duplicate row.
    pub fn suppress_self_writes(&self, duration: Duration) {
        *self.suppress_until.lock() = Some(Instant::now() + duration);
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

        // Baseline fingerprint so pre-existing clipboard content is not re-captured.
        // A transient busy clipboard here just means "no baseline" — the first poll
        // handles that case normally.
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(Some(captured)) = read_clipboard_text(&mut clipboard) {
                *last_text_fp.lock() = Some(captured.fingerprint());
            }
        }
        let last_seq = AtomicU32::new(clipboard_sequence_number());

        thread::spawn(move || {
            let poll_interval = Duration::from_millis(250);
            // Reuse handle across polls; recreate only after open failure
            let mut clipboard_slot: Option<Clipboard> = Clipboard::new().ok();
            info!(
                "Clipboard monitor started (poll every {}ms)",
                poll_interval.as_millis()
            );
            // Log a busy clipboard once per episode, not every 250ms tick.
            let mut busy_logged = false;

            while running.load(Ordering::SeqCst) {
                // Always refresh paste destination while user works in other apps.
                track_last_foreign_foreground();

                let seq = clipboard_sequence_number();
                // Sequence unchanged → skip all clipboard reads (esp. get_image RGBA copy)
                if seq != 0 && seq == last_seq.load(Ordering::Relaxed) {
                    thread::sleep(poll_interval);
                    continue;
                }
                // Do NOT advance `last_seq` yet. The watermark is committed only
                // after the clipboard is successfully opened below; otherwise a
                // transient `ClipboardOccupied` failure would consume this sequence
                // transition and the copy would be lost forever (the next poll sees
                // an unchanged sequence and skips).

                if clipboard_slot.is_none() {
                    clipboard_slot = Clipboard::new().ok();
                }
                let Some(clipboard) = clipboard_slot.as_mut() else {
                    thread::sleep(poll_interval);
                    continue;
                };

                // First open of this tick. If another process holds the clipboard
                // (common right after login/startup), leave the watermark untouched
                // so the next poll retries this same sequence transition.
                let text = match read_clipboard_text(clipboard) {
                    Ok(text) => text,
                    Err(e) => {
                        if !busy_logged {
                            warn!("Clipboard busy, deferring capture: {e}");
                            busy_logged = true;
                        }
                        thread::sleep(poll_interval);
                        continue;
                    }
                };
                // The sequence watermark is committed only after ALL clipboard
                // reads for this tick succeed. If any read hits ClipboardOccupied
                // we leave the watermark untouched so the next poll retries this
                // same sequence transition.

                let suppressed = is_capture_suppressed(&suppress_until);

                // Text was read first (above). Skip get_image() (full RGBA copy) when:
                // - meaningful share text wins over a co-existing thumb, or
                // - the clipboard has no bitmap/DIB formats at all.
                let prefer_text = text
                    .as_ref()
                    .map(|t| is_meaningful_share_text(&t.text))
                    .unwrap_or(false);

                if prefer_text {
                    if let Some(captured) = text {
                        maybe_emit_text(&last_text_fp, captured, suppressed, &on_change);
                    }
                    busy_logged = false;
                    last_seq.store(seq, Ordering::Relaxed);
                    thread::sleep(poll_interval);
                    continue;
                }

                if clipboard_has_bitmap_format() {
                    // Windows often keeps BOTH a bitmap and text:
                    // - Screenshots: image + empty/stub text → keep image
                    // - Browser "Copy image": image + URL-only text → keep image
                    let image = match clipboard.get_image() {
                        Err(e @ arboard::Error::ClipboardOccupied) => {
                            if !busy_logged {
                                warn!("Clipboard busy during image read, deferring: {e}");
                                busy_logged = true;
                            }
                            thread::sleep(poll_interval);
                            continue;
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
                            // Stale bitmap + new text (common on Windows) → still emit text.
                            if let Some(captured) = text {
                                maybe_emit_text(&last_text_fp, captured, suppressed, &on_change);
                            }
                        } else if suppressed {
                            // Same rule as text: do not advance fingerprints while
                            // suppressing, or a real copy in the window is lost forever.
                            debug!(
                                "Suppressed self-write image capture {}x{}",
                                img.width, img.height
                            );
                        } else {
                            let width = img.width as u32;
                            let height = img.height as u32;
                            // Prefer moving owned buffer; only copy when Cow is borrowed
                            let raw = match img.bytes {
                                std::borrow::Cow::Owned(v) => v,
                                std::borrow::Cow::Borrowed(b) => b.to_vec(),
                            };
                            // SHA-256 of full RGBA is done on the capture worker —
                            // poll only needs the cheap quick fingerprint for change detection.
                            *last_image_hash.lock() = Some(quick);
                            // Cap very large bitmaps BEFORE they enter the bounded channel:
                            // raw RGBA at 8K ≈ 660MB. We only need a 2560px-max edge for
                            // preview + paste; store_clipboard_image() also targets MAX_EDGE.
                            let (rgba, width, height) =
                                downscale_captured_rgba_if_large(raw, width, height);
                            debug!("Clipboard changed (image): {}x{}", width, height);
                            on_change(ClipboardEvent::Image(CapturedImage {
                                rgba,
                                width,
                                height,
                                hash: String::new(),
                            }));
                        }
                    } else if let Some(captured) = text {
                        maybe_emit_text(&last_text_fp, captured, suppressed, &on_change);
                    }
                } else if let Some(captured) = text {
                    maybe_emit_text(&last_text_fp, captured, suppressed, &on_change);
                }

                // All reads for this tick succeeded — commit the watermark.
                busy_logged = false;
                last_seq.store(seq, Ordering::Relaxed);
                thread::sleep(poll_interval);
            }

            debug!("Clipboard monitor stopped");
        });
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
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

/// H-2: Quick dedup fingerprint for clipboard images. Uses FNV-1a (non-crypto)
/// over dimensions + sampled head/tail bytes. This only guards the poll-loop
/// dedup check (last_image_hash); the authoritative content hash is computed
/// later by the image worker (SHA-256 over full RGBA).
/// Collision risk: two different images with identical size and matching edge
/// samples (e.g. large near-solid screenshots) may be treated as unchanged.
fn image_quick_fingerprint(img: &arboard::ImageData<'_>) -> String {
    let bytes = img.bytes.as_ref();
    // FNV-1a 64-bit: fast, no heap alloc, sufficient collision resistance for dedup.
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut h = FNV_OFFSET;
    let mut feed = |data: &[u8]| {
        for &b in data {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    feed(&img.width.to_le_bytes());
    feed(&img.height.to_le_bytes());
    feed(&(bytes.len() as u64).to_le_bytes());
    let n = bytes.len().min(2048);
    feed(&bytes[..n]);
    if bytes.len() > 4096 {
        feed(&bytes[bytes.len() - 2048..]);
    }
    format!("{:016x}", h)
}

/// Maximum edge (px) for a captured bitmap entering the process pipeline.
/// Mirrors `media::MAX_EDGE` so the on-disk file and in-memory buffer match.
const CAPTURE_MAX_EDGE: u32 = 2560;

/// Downscale an RGBA clipboard bitmap to at most `CAPTURE_MAX_EDGE` on its
/// longest side before it is moved into the bounded capture channel.
///
/// Without this, an 8K screenshot carries ~660MB of raw RGBA that sits in the
/// channel (capacity) plus the worker until PNG encoding completes — a real OOM
/// risk on memory-constrained machines. `arboard` guarantees RGBA byte order,
/// which matches `image::RgbaImage`.
fn downscale_captured_rgba_if_large(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> (Vec<u8>, u32, u32) {
    if width <= CAPTURE_MAX_EDGE && height <= CAPTURE_MAX_EDGE {
        return (rgba, width, height);
    }
    // Zero-sized bitmaps: nothing to downscale, pass through unchanged.
    if width == 0 || height == 0 {
        return (rgba, width, height);
    }
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    // arboard pixel buffers can carry trailing stride padding or be short from
    // some sources; normalize to width*height*4 before handing to the image crate.
    let mut pixels = rgba;
    if pixels.len() < expected {
        pixels.resize(expected, 0);
    } else if pixels.len() > expected {
        pixels.truncate(expected);
    }
    // After normalization pixels.len() == expected, so from_raw cannot fail.
    let img = match RgbaImage::from_raw(width, height, pixels) {
        Some(img) => img,
        None => {
            warn!(
                "Failed to wrap {}x{} RGBA buffer for downscale; sending as-is",
                width, height
            );
            return (Vec::new(), width, height);
        }
    };
    let scale = (CAPTURE_MAX_EDGE as f32 / width.max(height) as f32).min(1.0);
    let nw = ((width as f32) * scale).round().max(1.0) as u32;
    let nh = ((height as f32) * scale).round().max(1.0) as u32;
    let out = image::imageops::resize(&img, nw, nh, FilterType::Triangle);
    debug!(
        "Downscaled captured image {}x{} -> {}x{}",
        width, height, nw, nh
    );
    (out.into_raw(), nw, nh)
}

/// Read plain text (+ optional HTML) from the clipboard.
///
/// Returns `Err` only for a *transient* failure — the clipboard is held by
/// another process (`ClipboardOccupied`) — so the caller can keep the sequence
/// watermark and retry on the next tick. A clipboard that opened fine but holds
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
    let start = ["https://", "http://", "ftp://"]
        .iter()
        .filter_map(|p| lower.find(p))
        .min();
    let Some(start) = start else {
        return false;
    };
    let before = t[..start].trim();
    let from_url = &t[start..];
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
    suppressed: bool,
    on_change: &impl Fn(ClipboardEvent),
) {
    let fp = captured.fingerprint();
    let should_notify = {
        let last = last_text_fp.lock();
        match &*last {
            Some(prev) if prev == &fp => false,
            _ => !captured.text.trim().is_empty(),
        }
    };
    // During paste suppress: skip emit but do NOT advance last_text_fp.
    // Advancing it would permanently drop a real copy that lands in the window
    // (fingerprint already matches "seen", so it never emits after suppress ends).
    // Re-capture of our own paste after the window is fine — DB hash dedupes it.
    if should_notify && suppressed {
        debug!(
            "Suppressed self-write text capture: {} chars, html={}",
            captured.text.len(),
            captured.html.is_some()
        );
        return;
    }
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

/// Simulate Ctrl+V after clipboard content has been set.
/// Caller should delay (~80ms) after focusing the target so the window is ready.
#[cfg(windows)]
pub fn simulate_paste_keys() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    unsafe {
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Focus delay + key simulation (blocking). Prefer async sleep + [`simulate_paste_keys`]
/// on the Tauri command path so the blocking pool is not held during sleep.
#[cfg(windows)]
#[allow(dead_code)]
pub fn simulate_paste() {
    thread::sleep(Duration::from_millis(80));
    simulate_paste_keys();
}

#[cfg(not(windows))]
pub fn simulate_paste_keys() {
    warn!("Paste simulation not available on this platform");
}

#[cfg(not(windows))]
pub fn simulate_paste() {
    warn!("Paste simulation not available on this platform");
}

/// Paste target HWND: last non-ClipVault foreground window (kept fresh by tracker).
static PASTE_TARGET_HWND: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);
/// Our main window HWND (tao/WebView2 top-level). Needed because FG may be owned by
/// the WebView2 process (different PID) yet still be our UI.
static OUR_MAIN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(windows)]
fn root_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOT};
    unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        if root.is_null() {
            hwnd
        } else {
            root
        }
    }
}

/// Cache our main HWND so the background tracker can ignore WebView2-hosted FG.
pub fn set_our_main_hwnd(hwnd: Option<isize>) {
    OUR_MAIN_HWND.store(hwnd.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
}

fn our_main_hwnd() -> Option<isize> {
    let v = OUR_MAIN_HWND.load(std::sync::atomic::Ordering::SeqCst);
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

/// True if `hwnd` is our panel (same process, same root, or WebView2 child of our window).
#[cfg(windows)]
pub fn hwnd_belongs_to_us(
    hwnd: windows_sys::Win32::Foundation::HWND,
    our_hwnd: Option<isize>,
) -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetParent, GetWindowThreadProcessId,
    };

    if hwnd.is_null() {
        return false;
    }
    let our = our_hwnd.or_else(our_main_hwnd);
    unsafe {
        if let Some(our) = our {
            let our_h = our as windows_sys::Win32::Foundation::HWND;
            if !our_h.is_null() {
                if hwnd == our_h || root_hwnd(hwnd) == root_hwnd(our_h) {
                    return true;
                }
                // Walk parents — WebView2 host HWNDs sit under our top-level frame.
                let mut cur = hwnd;
                for _ in 0..24 {
                    if cur.is_null() {
                        break;
                    }
                    if cur == our_h || root_hwnd(cur) == root_hwnd(our_h) {
                        return true;
                    }
                    let parent = GetParent(cur);
                    if parent.is_null() || parent == cur {
                        break;
                    }
                    cur = parent;
                }
            }
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid != 0 && pid == GetCurrentProcessId()
    }
}

/// Continuously record the last foreign foreground window.
#[cfg(windows)]
pub fn track_last_foreign_foreground() {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use std::sync::atomic::{AtomicIsize, Ordering};

    // H-1: Cache last-seen foreground HWND. The expensive hwnd_belongs_to_us
    // parent-walk (up to 24 levels) only runs when the user actually switches
    // windows, not on every 250ms poll tick.
    static LAST_SEEN_FG: AtomicIsize = AtomicIsize::new(0);

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return;
        }
        let fg_val = fg as isize;
        // Fast path: same window as last tick → skip all further work.
        if LAST_SEEN_FG.load(Ordering::Relaxed) == fg_val {
            return;
        }
        LAST_SEEN_FG.store(fg_val, Ordering::Relaxed);

        if hwnd_belongs_to_us(fg, our_main_hwnd()) {
            return;
        }
        let target = root_hwnd(fg) as isize;
        let mut slot = PASTE_TARGET_HWND.lock();
        if *slot != Some(target) {
            *slot = Some(target);
            debug!("Tracked paste target hwnd={:#x}", target);
        }
    }
}

#[cfg(not(windows))]
pub fn track_last_foreign_foreground() {}

/// Snapshot the current foreground window as the paste destination, unless it is us.
#[cfg(windows)]
pub fn remember_paste_target(our_hwnd: Option<isize>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    if our_hwnd.is_some() {
        set_our_main_hwnd(our_hwnd);
    }

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return;
        }
        if hwnd_belongs_to_us(fg, our_hwnd) {
            return;
        }
        let target = root_hwnd(fg) as isize;
        *PASTE_TARGET_HWND.lock() = Some(target);
        debug!("Remembered paste target hwnd={:#x}", target);
    }
}

#[cfg(not(windows))]
pub fn remember_paste_target(_our_hwnd: Option<isize>) {}

#[cfg(windows)]
#[allow(dead_code)]
pub fn paste_target_hwnd() -> Option<isize> {
    *PASTE_TARGET_HWND.lock()
}

#[cfg(not(windows))]
pub fn paste_target_hwnd() -> Option<isize> {
    None
}

/// After we hide, Windows may already restore the previous app — enough for Ctrl+V.
#[cfg(windows)]
pub fn foreground_is_pasteable(our_hwnd: Option<isize>) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let fg = GetForegroundWindow();
        !fg.is_null() && !hwnd_belongs_to_us(fg, our_hwnd.or_else(our_main_hwnd))
    }
}

#[cfg(not(windows))]
pub fn foreground_is_pasteable(_our_hwnd: Option<isize>) -> bool {
    false
}

/// Bring a window to the foreground so simulated Ctrl+V lands in it.
/// Call while we still hold foreground rights (before hide) when possible.
#[cfg(windows)]
pub fn focus_window(hwnd_id: isize) -> bool {
    use windows_sys::Win32::Foundation::{FALSE, TRUE};
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
        IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SwitchToThisWindow, SystemParametersInfoW,
        SPI_GETFOREGROUNDLOCKTIMEOUT, SPI_SETFOREGROUNDLOCKTIMEOUT, SPIF_SENDCHANGE,
        SPIF_UPDATEINIFILE, SW_RESTORE,
    };

    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return false;
        }
        let hwnd = root_hwnd(hwnd);

        let fg_now = GetForegroundWindow();
        if !fg_now.is_null() && root_hwnd(fg_now) == hwnd {
            return true;
        }

        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        // Bypass foreground lock timeout (clipboard-manager standard technique).
        let mut lock_timeout: u32 = 0;
        let _ = SystemParametersInfoW(
            SPI_GETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut lock_timeout as *mut u32 as *mut _,
            0,
        );
        let mut zero: u32 = 0;
        let _ = SystemParametersInfoW(
            SPI_SETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut zero as *mut u32 as *mut _,
            SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
        );

        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        let mut target_pid = 0u32;
        let fg_tid = if !fg.is_null() {
            GetWindowThreadProcessId(fg, &mut fg_pid)
        } else {
            0
        };
        let target_tid = GetWindowThreadProcessId(hwnd, &mut target_pid);
        let cur_tid = GetCurrentThreadId();

        keybd_event(VK_MENU as u8, 0, 0, 0);
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);

        if target_pid != 0 {
            AllowSetForegroundWindow(target_pid);
        }
        AllowSetForegroundWindow(u32::MAX);

        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, TRUE);
        }
        if target_tid != 0 && target_tid != cur_tid && target_tid != fg_tid {
            AttachThreadInput(cur_tid, target_tid, TRUE);
        }

        BringWindowToTop(hwnd);
        SwitchToThisWindow(hwnd, TRUE);
        let _ = SetForegroundWindow(hwnd);

        if target_tid != 0 && target_tid != cur_tid && target_tid != fg_tid {
            AttachThreadInput(cur_tid, target_tid, FALSE);
        }
        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, FALSE);
        }

        let _ = SystemParametersInfoW(
            SPI_SETFOREGROUNDLOCKTIMEOUT,
            0,
            &mut lock_timeout as *mut u32 as *mut _,
            SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
        );

        let now = GetForegroundWindow();
        let ok = !now.is_null() && root_hwnd(now) == hwnd;
        if ok {
            debug!("Focused paste target hwnd={:#x}", hwnd as isize);
        } else {
            warn!(
                "SetForegroundWindow failed for hwnd={:#x} (fg={:#x})",
                hwnd as isize,
                now as isize
            );
        }
        ok
    }
}

#[cfg(not(windows))]
pub fn focus_window(_hwnd_id: isize) -> bool {
    false
}

/// Whether `hwnd_id` (or its root owner) is the current foreground window.
#[cfg(windows)]
pub fn is_foreground_hwnd(hwnd_id: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};
    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return false;
        }
        let fg = GetForegroundWindow();
        !fg.is_null() && root_hwnd(fg) == root_hwnd(hwnd)
    }
}

#[cfg(not(windows))]
pub fn is_foreground_hwnd(_hwnd_id: isize) -> bool {
    false
}

/// Hide via Win32 so focus can leave us even if Tauri hide is delayed.
#[cfg(windows)]
pub fn hide_hwnd(hwnd_id: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let hwnd = hwnd_id as windows_sys::Win32::Foundation::HWND;
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub fn hide_hwnd(_hwnd_id: isize) {}

/// Prefer the tracked foreign window. Fall back to current FG if foreign.
#[cfg(windows)]
pub fn resolve_paste_target(our_hwnd: Option<isize>) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};

    if our_hwnd.is_some() {
        set_our_main_hwnd(our_hwnd);
    }
    track_last_foreign_foreground();

    let our = our_hwnd.or_else(our_main_hwnd);
    let remembered = *PASTE_TARGET_HWND.lock();
    if let Some(id) = remembered {
        let hwnd = id as windows_sys::Win32::Foundation::HWND;
        unsafe {
            if !hwnd.is_null() && IsWindow(hwnd) != 0 && !hwnd_belongs_to_us(hwnd, our) {
                return Some(root_hwnd(hwnd) as isize);
            }
        }
    }

    unsafe {
        let fg = GetForegroundWindow();
        if !fg.is_null() && !hwnd_belongs_to_us(fg, our) {
            let id = root_hwnd(fg) as isize;
            *PASTE_TARGET_HWND.lock() = Some(id);
            return Some(id);
        }
    }
    None
}

#[cfg(not(windows))]
pub fn resolve_paste_target(_our_hwnd: Option<isize>) -> Option<isize> {
    None
}

/// Write plain text to the clipboard (no key simulation).
#[cfg(windows)]
pub fn write_clipboard_plain(text: &str) -> bool {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Err(e) = clipboard.set_text(text) {
            warn!("Failed to set clipboard for paste: {}", e);
            return false;
        }
        return true;
    }
    false
}

#[cfg(not(windows))]
pub fn write_clipboard_plain(_text: &str) -> bool {
    false
}

/// Write text (+ optional HTML) to the clipboard (no key simulation).
#[cfg(windows)]
pub fn write_clipboard_text(text: &str, html: Option<&str>) -> bool {
    if let Ok(mut clipboard) = Clipboard::new() {
        let ok = if let Some(h) = html.filter(|s| !s.trim().is_empty()) {
            match clipboard.set_html(h, Some(text)) {
                Ok(()) => true,
                Err(e) => {
                    warn!("Failed to set HTML clipboard, falling back to text: {}", e);
                    clipboard.set_text(text).is_ok()
                }
            }
        } else {
            clipboard.set_text(text).is_ok()
        };
        if !ok {
            warn!("Failed to set clipboard for paste");
        }
        return ok;
    }
    false
}

#[cfg(not(windows))]
pub fn write_clipboard_text(_text: &str, _html: Option<&str>) -> bool {
    false
}

/// Write raw PNG file bytes as the registered "PNG" clipboard format.
/// Avoids decoding to RGBA — much cheaper for large images. Many modern apps
/// (Chrome, Office, Discord, etc.) accept this; callers should fall back to
/// [`write_clipboard_image`] for CF_DIB-only targets.
#[cfg(windows)]
pub fn write_clipboard_png_file(path: &std::path::Path) -> bool {
    use std::fs;
    use std::ptr;
    use windows_sys::Win32::Foundation::HGLOBAL;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    // windows-sys 0.59 omits GlobalFree; still required to release failed allocations.
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalFree(hmem: HGLOBAL) -> HGLOBAL;
    }

    let bytes = match fs::read(path) {
        Ok(b) if !b.is_empty() => b,
        Ok(_) => {
            warn!("PNG file empty for clipboard: {}", path.display());
            return false;
        }
        Err(e) => {
            warn!("Failed to read PNG for clipboard ({}): {}", path.display(), e);
            return false;
        }
    };

    let fmt_name: Vec<u16> = "PNG".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let fmt = RegisterClipboardFormatW(fmt_name.as_ptr());
        if fmt == 0 {
            warn!("RegisterClipboardFormatW(PNG) failed");
            return false;
        }

        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        if hmem.is_null() {
            warn!("GlobalAlloc failed for PNG clipboard ({} bytes)", bytes.len());
            return false;
        }

        let locked = GlobalLock(hmem);
        if locked.is_null() {
            GlobalFree(hmem);
            warn!("GlobalLock failed for PNG clipboard");
            return false;
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), locked as *mut u8, bytes.len());
        GlobalUnlock(hmem);

        if OpenClipboard(ptr::null_mut()) == 0 {
            GlobalFree(hmem);
            warn!("OpenClipboard failed for PNG paste");
            return false;
        }
        EmptyClipboard();
        let set = SetClipboardData(fmt, hmem);
        CloseClipboard();
        if set.is_null() {
            GlobalFree(hmem);
            warn!("SetClipboardData(PNG) failed");
            return false;
        }
        // System owns hmem after successful SetClipboardData.
        true
    }
}

#[cfg(not(windows))]
pub fn write_clipboard_png_file(_path: &std::path::Path) -> bool {
    false
}

/// Write an RGBA image to the clipboard (no key simulation).
#[cfg(windows)]
pub fn write_clipboard_image(rgba: &[u8], width: usize, height: usize) -> bool {
    if let Ok(mut clipboard) = Clipboard::new() {
        let img = ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Borrowed(rgba),
        };
        if let Err(e) = clipboard.set_image(img) {
            warn!("Failed to set clipboard image for paste: {}", e);
            return false;
        }
        return true;
    }
    false
}

#[cfg(not(windows))]
pub fn write_clipboard_image(_rgba: &[u8], _width: usize, _height: usize) -> bool {
    false
}

/// Capture the foreground window's title and module name (Windows only).
/// Cached briefly so bursty clipboard events don't OpenProcess every time.
#[cfg(windows)]
pub fn get_foreground_window_info() -> (String, String) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static CACHE: Mutex<Option<(Instant, String, String)>> = Mutex::new(None);
    const TTL: Duration = Duration::from_millis(250);

    if let Ok(guard) = CACHE.lock() {
        if let Some((at, title, app)) = guard.as_ref() {
            if at.elapsed() < TTL {
                return (title.clone(), app.clone());
            }
        }
    }

    let info = get_foreground_window_info_uncached();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), info.0.clone(), info.1.clone()));
    }
    info
}

#[cfg(windows)]
fn get_foreground_window_info_uncached() -> (String, String) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows_sys::Win32::Foundation::CloseHandle;
    // PROCESS_QUERY_LIMITED_INFORMATION + QueryFullProcessImageNameW works across
    // integrity levels without PROCESS_VM_READ. GetModuleFileNameW is wrong here —
    // it expects an HMODULE in *this* process, not a foreign process HANDLE.
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (String::new(), String::new());
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = if len > 0 {
            OsString::from_wide(&title_buf[..len as usize])
                .to_string_lossy()
                .to_string()
        } else {
            String::new()
        };

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return (title, String::new());
        }

        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process_handle.is_null() {
            return (title, String::new());
        }

        let mut module_buf = [0u16; 260];
        let mut size = module_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process_handle,
            0,
            module_buf.as_mut_ptr(),
            &mut size,
        );
        CloseHandle(process_handle);

        let module = if ok != 0 && size > 0 {
            let path = OsString::from_wide(&module_buf[..size as usize])
                .to_string_lossy()
                .to_string();
            std::path::Path::new(&path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(path)
        } else {
            String::new()
        };

        (title, module)
    }
}

#[cfg(not(windows))]
pub fn get_foreground_window_info() -> (String, String) {
    (String::new(), String::new())
}

#[cfg(test)]
mod tests {
    use super::{is_meaningful_share_text, is_primarily_url};

    #[test]
    fn url_only_is_primarily_url() {
        assert!(is_primarily_url("https://example.com/path"));
        assert!(is_primarily_url("see https://example.com"));
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
        assert!(!is_meaningful_share_text("https://example.com/some/long/path"));
    }
}
