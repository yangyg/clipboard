use image::imageops::FilterType;
use image::{ImageEncoder, ImageFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const THUMB_MAX_EDGE: u32 = 160;
const MAX_EDGE: u32 = 2560;

pub struct StoredImage {
    /// Relative path e.g. `media/{hash}.png`
    pub media_path: String,
    /// Relative path e.g. `media/thumbs/{hash}.jpg`
    pub thumb_path: String,
    pub width: u32,
    pub height: u32,
}

/// Ensure media directories exist under the app data root.
pub fn ensure_dirs(app_data_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(app_data_dir.join("media").join("thumbs"))?;
    Ok(())
}

pub fn absolute(app_data_dir: &Path, relative: &str) -> PathBuf {
    // Join segment-by-segment so Windows paths don't keep mixed `/` from relative keys.
    relative
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .fold(app_data_dir.to_path_buf(), |acc, part| acc.join(part))
}

/// Encode RGBA clipboard image to PNG (+ JPEG thumb) under media/.
/// Downscales if either edge exceeds MAX_EDGE.
pub fn store_clipboard_image(
    app_data_dir: &Path,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    hash: &str,
) -> Result<StoredImage, String> {
    ensure_dirs(app_data_dir).map_err(|e| e.to_string())?;

    let media_rel = format!("media/{hash}.png");
    let thumb_rel = format!("media/thumbs/{hash}.jpg");
    let media_abs = absolute(app_data_dir, &media_rel);
    let thumb_abs = absolute(app_data_dir, &thumb_rel);

    // Already stored (dedup hit at file level) — no size-cache bump.
    if media_abs.exists() && thumb_abs.exists() {
        let (w, h) = image::image_dimensions(&media_abs)
            .map(|(w, h)| (w, h))
            .unwrap_or((width, height));
        return Ok(StoredImage {
            media_path: media_rel,
            thumb_path: thumb_rel,
            width: w,
            height: h,
        });
    }

    // Clipboard RGBA may include stride padding or truncation from some apps
    let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    let mut pixels = rgba; // take ownership — avoid second full copy
    if pixels.len() < expected {
        pixels.resize(expected, 0);
    } else if pixels.len() > expected {
        pixels.truncate(expected);
    }
    let mut img = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| "Failed to create RGBA image".to_string())?;

    let (out_w, out_h) = if width > MAX_EDGE || height > MAX_EDGE {
        let scale = (MAX_EDGE as f32 / width.max(height) as f32).min(1.0);
        let nw = ((width as f32) * scale).round().max(1.0) as u32;
        let nh = ((height as f32) * scale).round().max(1.0) as u32;
        img = image::imageops::resize(&img, nw, nh, FilterType::Triangle);
        (nw, nh)
    } else {
        (width, height)
    };

    // Main PNG
    {
        let mut buf = Cursor::new(Vec::new());
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        encoder
            .write_image(img.as_raw(), out_w, out_h, image::ColorType::Rgba8.into())
            .map_err(|e| format!("PNG encode error: {e}"))?;
        fs::write(&media_abs, buf.into_inner()).map_err(|e| e.to_string())?;
    }

    // Thumbnail JPEG
    {
        let scale = (THUMB_MAX_EDGE as f32 / out_w.max(out_h) as f32).min(1.0);
        let tw = ((out_w as f32) * scale).round().max(1.0) as u32;
        let th = ((out_h as f32) * scale).round().max(1.0) as u32;
        let thumb = image::imageops::resize(&img, tw, th, FilterType::Triangle);
        let rgb = image::DynamicImage::ImageRgba8(thumb).to_rgb8();
        let mut buf = Cursor::new(Vec::new());
        rgb.write_to(&mut buf, ImageFormat::Jpeg)
            .map_err(|e| format!("JPEG thumb encode error: {e}"))?;
        fs::write(&thumb_abs, buf.into_inner()).map_err(|e| e.to_string())?;
    }

    let added = file_len(&media_abs).saturating_add(file_len(&thumb_abs));
    bump_media_dir_size_cache(app_data_dir, added as i64);

    debug!("Stored image {} ({}x{})", hash, out_w, out_h);
    Ok(StoredImage {
        media_path: media_rel,
        thumb_path: thumb_rel,
        width: out_w,
        height: out_h,
    })
}

pub fn delete_media_files(app_data_dir: &Path, media_path: Option<&str>, thumb_path: Option<&str>) {
    let mut removed: u64 = 0;
    for rel in [media_path, thumb_path].into_iter().flatten() {
        let path = absolute(app_data_dir, rel);
        if path.exists() {
            removed = removed.saturating_add(file_len(&path));
            if let Err(e) = fs::remove_file(&path) {
                warn!("Failed to delete media file {:?}: {}", path, e);
            }
        }
    }
    if removed > 0 {
        bump_media_dir_size_cache(app_data_dir, -(removed as i64));
    }
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

// --- media directory size cache (shared with get_stats) ---

struct MediaSizeCache {
    at: Instant,
    bytes: i64,
    root: PathBuf,
}

static MEDIA_SIZE_CACHE: StdMutex<Option<MediaSizeCache>> = StdMutex::new(None);
const MEDIA_SIZE_TTL: Duration = Duration::from_secs(120);

/// Walk `media/` once per TTL; writes/deletes adjust the cached total in place.
pub fn cached_media_dir_size(root: &Path) -> i64 {
    if let Ok(guard) = MEDIA_SIZE_CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.root == root && c.at.elapsed() < MEDIA_SIZE_TTL {
                return c.bytes;
            }
        }
    }
    let bytes = media_dir_size(root);
    if let Ok(mut guard) = MEDIA_SIZE_CACHE.lock() {
        *guard = Some(MediaSizeCache {
            at: Instant::now(),
            bytes,
            root: root.to_path_buf(),
        });
    }
    bytes
}

fn bump_media_dir_size_cache(root: &Path, delta: i64) {
    if let Ok(mut guard) = MEDIA_SIZE_CACHE.lock() {
        if let Some(c) = guard.as_mut() {
            if c.root == root {
                c.bytes = c.bytes.saturating_add(delta).max(0);
                return;
            }
        }
        // No live cache for this root — next stats call will walk.
        *guard = None;
    }
}

fn media_dir_size(root: &Path) -> i64 {
    fn walk(dir: &Path, acc: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, acc);
            } else if let Ok(meta) = entry.metadata() {
                *acc = acc.saturating_add(meta.len());
            }
        }
    }
    let mut total = 0u64;
    let media = root.join("media");
    if media.is_dir() {
        walk(&media, &mut total);
    } else {
        walk(root, &mut total);
    }
    total.min(i64::MAX as u64) as i64
}

/// Load PNG from disk into RGBA bytes for arboard set_image.
pub fn load_image_rgba(app_data_dir: &Path, media_path: &str) -> Result<(Vec<u8>, usize, usize), String> {
    let path = absolute(app_data_dir, media_path);
    let dyn_img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;
    let rgba = dyn_img.to_rgba8();
    let w = rgba.width() as usize;
    let h = rgba.height() as usize;
    Ok((rgba.into_raw(), w, h))
}
