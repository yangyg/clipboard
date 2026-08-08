//! Clipboard monitoring, paste-target tracking, and Win32 clipboard I/O.
//!
//! Split by responsibility to keep each file under the size cap:
//! - `monitor.rs` — arboard poll loop + text/event types
//! - `image.rs` — cheap image fingerprint + pre-channel downscaling
//! - `paste.rs` — paste-target HWND tracking, focus + Ctrl+V simulation
//! - `write.rs` — clipboard write helpers (text/HTML/PNG/RGBA)
//! - `fgwin.rs` — foreground window title + module name
//!
//! Public symbols are re-exported here so existing `crate::clipboard::*`
//! callers (lib.rs / commands.rs / tray.rs) stay unchanged.

mod fgwin;
mod image;
mod monitor;
mod paste;
mod write;

pub use fgwin::get_foreground_window_info;
pub use image::image_quick_fingerprint_rgba;
pub use monitor::{CapturedImage, CapturedText, ClipboardEvent, ClipboardMonitor};
pub use paste::{
    focus_window, foreground_is_pasteable, hide_hwnd, is_foreground_hwnd, remember_paste_target,
    resolve_paste_target, set_our_main_hwnd, simulate_paste_keys, track_last_foreign_foreground,
};
pub use write::{
    write_clipboard_image, write_clipboard_plain, write_clipboard_png_file, write_clipboard_text,
};
