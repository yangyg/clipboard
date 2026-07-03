use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::ImageEncoder;

#[derive(Debug, Clone)]
pub enum ClipboardEvent {
    Text(String),
    Image(String), // base64-encoded PNG
}

pub struct ClipboardMonitor {
    running: Arc<AtomicBool>,
    last_text: Arc<parking_lot::Mutex<Option<String>>>,
    last_image_hash: Arc<parking_lot::Mutex<Option<String>>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            last_text: Arc::new(parking_lot::Mutex::new(None)),
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
        let last_text = self.last_text.clone();
        let last_image_hash = self.last_image_hash.clone();

        // Get initial clipboard content
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                *last_text.lock() = Some(text);
            }
        }

        thread::spawn(move || {
            let poll_interval = Duration::from_millis(500);

            while running.load(Ordering::SeqCst) {
                match Clipboard::new() {
                    Ok(mut clipboard) => {
                        // --- Text check ---
                        let text_changed = match clipboard.get_text() {
                            Ok(text) => {
                                let text_clone = text.clone();
                                let should_notify = {
                                    let mut last = last_text.lock();
                                    match &*last {
                                        Some(prev) if prev == &text => false,
                                        _ => {
                                            if !text.trim().is_empty() {
                                                *last = Some(text);
                                                true
                                            } else {
                                                *last = Some(text);
                                                false
                                            }
                                        }
                                    }
                                };
                                if should_notify {
                                    debug!("Clipboard changed (text): {} chars", text_clone.len());
                                    on_change(ClipboardEvent::Text(text_clone));
                                    true
                                } else {
                                    false
                                }
                            }
                            Err(_) => false,
                        };

                        // --- Image check (only if text didn't change) ---
                        if !text_changed {
                            if let Ok(img) = clipboard.get_image() {
                                let raw = img.bytes.to_vec();
                                // SHA-256 hash for dedup
                                let hash = {
                                    use sha2::{Sha256, Digest};
                                    let mut hasher = Sha256::new();
                                    hasher.update(&raw);
                                    hex::encode(hasher.finalize())
                                };
                                let should_notify = {
                                    let mut last_hash = last_image_hash.lock();
                                    match &*last_hash {
                                        Some(prev) if prev == &hash => false,
                                        _ => {
                                            *last_hash = Some(hash);
                                            true
                                        }
                                    }
                                };
                                if should_notify {
                                    // Encode raw RGBA to PNG, then base64
                                    if let Ok(encoded) = encode_png_base64(&raw, img.width as u32, img.height as u32) {
                                        debug!("Clipboard changed (image): {}x{}", img.width, img.height);
                                        on_change(ClipboardEvent::Image(encoded));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to access clipboard: {}", e);
                    }
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

fn encode_png_base64(rgba_data: &[u8], width: u32, height: u32) -> Result<String, String> {
    let img = image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
        .ok_or("Failed to create RGBA image")?;
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder
        .write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())
        .map_err(|e| format!("PNG encode error: {}", e))?;
    Ok(BASE64.encode(buf.into_inner()))
}

/// Paste text to the active window via clipboard
#[cfg(windows)]
pub fn paste_text(text: &str) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    if let Ok(mut clipboard) = Clipboard::new() {
        if let Err(e) = clipboard.set_text(text) {
            warn!("Failed to set clipboard for paste: {}", e);
            return;
        }

        thread::sleep(Duration::from_millis(80));

        unsafe {
            keybd_event(VK_CONTROL as u8, 0, 0, 0);
            keybd_event(VK_V as u8, 0, 0, 0);
            keybd_event(VK_V as u8, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
        }
    }
}

#[cfg(not(windows))]
pub fn paste_text(_text: &str) {
    warn!("Paste simulation not available on this platform");
}

pub fn paste_plain_text(text: &str) {
    paste_text(text);
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
            // Extract just the filename from the full path
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
