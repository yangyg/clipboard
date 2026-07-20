use arboard::{Clipboard, ImageData};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

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
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            last_text_fp: Arc::new(parking_lot::Mutex::new(None)),
            last_image_hash: Arc::new(parking_lot::Mutex::new(None)),
        }
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

        // Get initial clipboard content
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Some(captured) = read_clipboard_text(&mut clipboard) {
                *last_text_fp.lock() = Some(captured.fingerprint());
            }
        }
        let last_seq = AtomicU32::new(clipboard_sequence_number());

        thread::spawn(move || {
            let poll_interval = Duration::from_millis(500);
            // Reuse handle across polls; recreate only after open failure
            let mut clipboard_slot: Option<Clipboard> = Clipboard::new().ok();

            while running.load(Ordering::SeqCst) {
                let seq = clipboard_sequence_number();
                // Sequence unchanged → skip all clipboard reads (esp. get_image RGBA copy)
                if seq != 0 && seq == last_seq.load(Ordering::Relaxed) {
                    thread::sleep(poll_interval);
                    continue;
                }
                last_seq.store(seq, Ordering::Relaxed);

                if clipboard_slot.is_none() {
                    clipboard_slot = Clipboard::new().ok();
                }
                let Some(clipboard) = clipboard_slot.as_mut() else {
                    thread::sleep(poll_interval);
                    continue;
                };

                // Windows often puts BOTH a bitmap and text on the clipboard:
                // - Screenshots: image + empty/stub text → keep image
                // - Douyin/WeChat shares: thumb image + long share text → prefer text
                let image = clipboard.get_image().ok();
                let text = read_clipboard_text(clipboard);

                let prefer_text = text
                    .as_ref()
                    .map(|t| is_meaningful_share_text(&t.text))
                    .unwrap_or(false);

                if let Some(img) = image {
                    // Cheap fingerprint first — avoid full SHA-256 when bitmap unchanged
                    let quick = image_quick_fingerprint(&img);
                    let unchanged = {
                        let last = last_image_hash.lock();
                        matches!(&*last, Some(prev) if prev == &quick)
                    };

                    if unchanged {
                        if prefer_text {
                            if let Some(captured) = text {
                                maybe_emit_text(&last_text_fp, captured, &on_change);
                            }
                        } else if let Some(captured) = text {
                            *last_text_fp.lock() = Some(captured.fingerprint());
                        }
                    } else if prefer_text {
                        *last_image_hash.lock() = Some(quick);
                        if let Some(captured) = text {
                            maybe_emit_text(&last_text_fp, captured, &on_change);
                        }
                    } else {
                        let width = img.width as u32;
                        let height = img.height as u32;
                        // Prefer moving owned buffer; only copy when Cow is borrowed
                        let raw = match img.bytes {
                            std::borrow::Cow::Owned(v) => v,
                            std::borrow::Cow::Borrowed(b) => b.to_vec(),
                        };
                        let hash = {
                            use sha2::{Digest, Sha256};
                            let mut hasher = Sha256::new();
                            hasher.update(&raw);
                            hex::encode(hasher.finalize())
                        };
                        *last_image_hash.lock() = Some(quick);
                        debug!("Clipboard changed (image): {}x{}", width, height);
                        on_change(ClipboardEvent::Image(CapturedImage {
                            rgba: raw,
                            width,
                            height,
                            hash,
                        }));
                        if let Some(captured) = text {
                            *last_text_fp.lock() = Some(captured.fingerprint());
                        }
                    }
                } else if let Some(captured) = text {
                    maybe_emit_text(&last_text_fp, captured, &on_change);
                }

                thread::sleep(poll_interval);
            }

            debug!("Clipboard monitor stopped");
        });
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
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

/// Width/height/len + sampled bytes — enough to skip unchanged images cheaply.
fn image_quick_fingerprint(img: &arboard::ImageData<'_>) -> String {
    use sha2::{Digest, Sha256};
    let bytes = img.bytes.as_ref();
    let mut hasher = Sha256::new();
    hasher.update(img.width.to_le_bytes());
    hasher.update(img.height.to_le_bytes());
    hasher.update((bytes.len() as u64).to_le_bytes());
    let n = bytes.len().min(2048);
    hasher.update(&bytes[..n]);
    if bytes.len() > 4096 {
        hasher.update(&bytes[bytes.len() - 2048..]);
    }
    hex::encode(hasher.finalize())
}

fn read_clipboard_text(clipboard: &mut Clipboard) -> Option<CapturedText> {
    let text = clipboard.get_text().ok()?;
    let html = clipboard
        .get()
        .html()
        .ok()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());
    Some(CapturedText { text, html })
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
    on_change: &impl Fn(ClipboardEvent),
) {
    let fp = captured.fingerprint();
    let should_notify = {
        let mut last = last_text_fp.lock();
        match &*last {
            Some(prev) if prev == &fp => false,
            _ => {
                *last = Some(fp);
                !captured.text.trim().is_empty()
            }
        }
    };
    if should_notify {
        debug!(
            "Clipboard changed (text): {} chars, html={}",
            captured.text.len(),
            captured.html.is_some()
        );
        on_change(ClipboardEvent::Text(captured));
    }
}

/// Simulate Ctrl+V after clipboard content has been set.
#[cfg(windows)]
fn simulate_paste() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    thread::sleep(Duration::from_millis(80));

    unsafe {
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Paste plain text only.
#[cfg(windows)]
pub fn paste_plain_text(text: &str) {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Err(e) = clipboard.set_text(text) {
            warn!("Failed to set clipboard for paste: {}", e);
            return;
        }
        simulate_paste();
    }
}

#[cfg(not(windows))]
pub fn paste_plain_text(_text: &str) {
    warn!("Paste simulation not available on this platform");
}

/// Paste with HTML format when available (keeps bold/color/etc. in Word, browsers…).
#[cfg(windows)]
pub fn paste_text(text: &str, html: Option<&str>) {
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
            return;
        }
        simulate_paste();
    }
}

#[cfg(not(windows))]
pub fn paste_text(_text: &str, _html: Option<&str>) {
    warn!("Paste simulation not available on this platform");
}

/// Paste an RGBA image to the active window via clipboard
#[cfg(windows)]
pub fn paste_image(rgba: &[u8], width: usize, height: usize) {
    if let Ok(mut clipboard) = Clipboard::new() {
        let img = ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Borrowed(rgba),
        };
        if let Err(e) = clipboard.set_image(img) {
            warn!("Failed to set clipboard image for paste: {}", e);
            return;
        }
        simulate_paste();
    }
}

#[cfg(not(windows))]
pub fn paste_image(_rgba: &[u8], _width: usize, _height: usize) {
    warn!("Paste simulation not available on this platform");
}

/// Capture the foreground window's title and module name (Windows only)
#[cfg(windows)]
pub fn get_foreground_window_info() -> (String, String) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
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
        let process_handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            0,
            pid,
        );
        if process_handle.is_null() {
            return (title, String::new());
        }
        let mut module_buf = [0u16; 260];
        let mod_len = GetModuleFileNameW(process_handle, module_buf.as_mut_ptr(), module_buf.len() as u32);
        CloseHandle(process_handle);
        let module = if mod_len > 0 {
            let path = OsString::from_wide(&module_buf[..mod_len as usize])
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
