//! Clipboard write helpers (plain text / text+HTML / PNG file / RGBA image).
//! None of these simulate keys — callers drive focus and Ctrl+V separately.
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::path::Path;
use tracing::warn;

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
pub fn write_clipboard_png_file(path: &Path) -> bool {
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
pub fn write_clipboard_png_file(_path: &Path) -> bool {
    false
}

/// Write an RGBA image to the clipboard (no key simulation).
#[cfg(windows)]
pub fn write_clipboard_image(rgba: &[u8], width: usize, height: usize) -> bool {
    if let Ok(mut clipboard) = Clipboard::new() {
        let img = ImageData {
            width,
            height,
            bytes: Cow::Borrowed(rgba),
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
