//! Media file upload/download orchestration for WebDAV sync.
use std::fs;
use std::path::Path;

use super::bundle::ManifestEntry;
use super::client::WebDavClient;
use super::sync::join_remote;
use crate::media;
use crate::ClipboardRecord;

/// Server-supplied media rels must satisfy the same strict hash-path rule as
/// imports (`security::is_allowed_media_rel`). `media::absolute` alone strips
/// `..`, but a Windows drive-letter segment (`C:x`) still escapes the media
/// root — never let one reach `fs::write` / `fs::read`.
fn safe_media_rel(rel: &str) -> bool {
    crate::security::is_allowed_media_rel(rel)
}

fn write_downloaded_media(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let format = image::guess_format(bytes).map_err(|_| "远端媒体格式无效".to_string())?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("远端媒体只允许 PNG 或 JPEG".into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("media");
    let temp = path.with_file_name(format!(".{name}.download"));
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error.to_string());
    }
    Ok(())
}

pub(super) async fn download_media_if_needed(
    client: &WebDavClient,
    root: &str,
    media_root: &Path,
    entry: &ManifestEntry,
) -> Result<bool, String> {
    if !entry.has_media {
        return Ok(false);
    }
    let mut downloaded = false;
    for rel in [entry.media_path.as_deref(), entry.thumb_path.as_deref()] {
        let Some(rel) = rel.filter(|p| !p.is_empty()) else {
            continue;
        };
        if !safe_media_rel(rel) {
            continue;
        }
        let abs = media::absolute(media_root, rel);
        if abs.exists() {
            continue;
        }
        let remote = join_remote(root, rel);
        if let Some(bytes) = client.get_bytes(&remote).await? {
            if let Some(parent) = abs.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_downloaded_media(&abs, &bytes)?;
            downloaded = true;
        }
    }
    Ok(downloaded)
}

pub(super) async fn upload_media_if_needed(
    client: &WebDavClient,
    root: &str,
    media_root: &Path,
    rec: &ClipboardRecord,
) -> Result<(bool, bool), String> {
    // returns (uploaded, skipped)
    let Some(media_rel) = rec.media_path.as_deref().filter(|p| !p.is_empty()) else {
        return Ok((false, false));
    };
    // Rels can be server-supplied (remote records flow through the catalog) —
    // enforce the strict hash-path rule before any fs::read to block
    // remote-directed file exfiltration.
    if !safe_media_rel(media_rel) {
        return Ok((false, false));
    }
    let abs = media::absolute(media_root, media_rel);
    if !abs.exists() {
        return Ok((false, false));
    }
    let remote = join_remote(root, media_rel);
    if client.exists(&remote).await? {
        // still ensure thumb if missing remotely
        let mut skipped = true;
        let mut uploaded = false;
        if let Some(thumb_rel) = rec.thumb_path.as_deref().filter(|p| !p.is_empty()) {
            if safe_media_rel(thumb_rel) {
                let thumb_abs = media::absolute(media_root, thumb_rel);
                let thumb_remote = join_remote(root, thumb_rel);
                if thumb_abs.exists() && !client.exists(&thumb_remote).await? {
                    let bytes = fs::read(&thumb_abs).map_err(|e| e.to_string())?;
                    client.put_bytes(&thumb_remote, bytes, "image/jpeg").await?;
                    uploaded = true;
                    skipped = false;
                }
            }
        }
        return Ok((uploaded, skipped));
    }
    let bytes = fs::read(&abs).map_err(|e| e.to_string())?;
    let ct = if media_rel.ends_with(".png") {
        "image/png"
    } else if media_rel.ends_with(".jpg") || media_rel.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };
    client.put_bytes(&remote, bytes, ct).await?;
    if let Some(thumb_rel) = rec.thumb_path.as_deref().filter(|p| !p.is_empty()) {
        if safe_media_rel(thumb_rel) {
            let thumb_abs = media::absolute(media_root, thumb_rel);
            if thumb_abs.exists() {
                let thumb_remote = join_remote(root, thumb_rel);
                if !client.exists(&thumb_remote).await? {
                    let tbytes = fs::read(&thumb_abs).map_err(|e| e.to_string())?;
                    client
                        .put_bytes(&thumb_remote, tbytes, "image/jpeg")
                        .await?;
                }
            }
        }
    }
    Ok((true, false))
}
