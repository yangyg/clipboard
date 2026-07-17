use image::imageops::FilterType;
use image::{ImageEncoder, ImageFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

const THUMB_MAX_EDGE: u32 = 240;
const MAX_EDGE: u32 = 4096;

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
    app_data_dir.join(relative)
}

/// Encode RGBA clipboard image to PNG (+ JPEG thumb) under media/.
/// Downscales if either edge exceeds MAX_EDGE.
pub fn store_clipboard_image(
    app_data_dir: &Path,
    rgba: &[u8],
    width: u32,
    height: u32,
    hash: &str,
) -> Result<StoredImage, String> {
    ensure_dirs(app_data_dir).map_err(|e| e.to_string())?;

    let media_rel = format!("media/{hash}.png");
    let thumb_rel = format!("media/thumbs/{hash}.jpg");
    let media_abs = absolute(app_data_dir, &media_rel);
    let thumb_abs = absolute(app_data_dir, &thumb_rel);

    // Already stored (dedup hit at file level)
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

    let mut img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
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

    debug!("Stored image {} ({}x{})", hash, out_w, out_h);
    Ok(StoredImage {
        media_path: media_rel,
        thumb_path: thumb_rel,
        width: out_w,
        height: out_h,
    })
}

pub fn delete_media_files(app_data_dir: &Path, media_path: Option<&str>, thumb_path: Option<&str>) {
    for rel in [media_path, thumb_path].into_iter().flatten() {
        let path = absolute(app_data_dir, rel);
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                warn!("Failed to delete media file {:?}: {}", path, e);
            }
        }
    }
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
