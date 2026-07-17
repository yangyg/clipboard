use arboard::{Clipboard, ImageData};
use std::sync::atomic::{AtomicBool, Ordering};
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
pub enum ClipboardEvent {
    Text(String),
    Image(CapturedImage),
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
                                            *last_hash = Some(hash.clone());
                                            true
                                        }
                                    }
                                };
                                if should_notify {
                                    debug!(
                                        "Clipboard changed (image): {}x{}",
                                        img.width, img.height
                                    );
                                    on_change(ClipboardEvent::Image(CapturedImage {
                                        rgba: raw,
                                        width: img.width as u32,
                                        height: img.height as u32,
                                        hash,
                                    }));
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

/// Paste text to the active window via clipboard
#[cfg(windows)]
pub fn paste_text(text: &str) {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Err(e) = clipboard.set_text(text) {
            warn!("Failed to set clipboard for paste: {}", e);
            return;
        }
        simulate_paste();
    }
}

#[cfg(not(windows))]
pub fn paste_text(_text: &str) {
    warn!("Paste simulation not available on this platform");
}

pub fn paste_plain_text(text: &str) {
    paste_text(text);
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
