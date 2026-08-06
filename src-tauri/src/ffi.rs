//! Windows FFI declarations that `windows-sys` 0.59 omits.
//!
//! Both functions release memory Windows allocated on our behalf. Declared in
//! one place so the security (DPAPI) and clipboard (PNG) code paths share them
//! instead of repeating `extern "system"` blocks.

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    /// Release a `CryptProtectData` / `CryptUnprotectData` output blob.
    pub(crate) fn LocalFree(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    /// Release a `GMEM_MOVEABLE` handle that the clipboard did not take ownership of.
    pub(crate) fn GlobalFree(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}
