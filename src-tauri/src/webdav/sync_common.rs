//! Shared WebDAV setup: remote paths, client, device id, remote snapshot, persist.

use std::sync::Arc;

use chrono::Utc;

use crate::db::ClipboardDb;
use crate::Settings;

use super::bundle::{parse_bundle, SyncManifest, BUNDLE_REL, MANIFEST_NAME};
use super::client::WebDavClient;

pub(super) fn remote_root(settings: &Settings) -> String {
    let p = settings.webdav_remote_path.trim().trim_matches('/');
    if p.is_empty() {
        "ClipVaultSync".into()
    } else {
        p.to_string()
    }
}

/// Join a remote path segment onto the sync root, tolerating stray slashes.
pub(super) fn join_remote(root: &str, rel: &str) -> String {
    format!(
        "{}/{}",
        root.trim_end_matches('/'),
        rel.trim_start_matches('/')
    )
}

pub(super) fn ensure_device_id(settings: &mut Settings) -> String {
    if settings.webdav_device_id.trim().is_empty() {
        settings.webdav_device_id = uuid::Uuid::new_v4().to_string();
    }
    settings.webdav_device_id.clone()
}

/// Persist settings with a fresh `webdav_last_sync_at` stamp, off the async worker.
pub(super) async fn persist_last_sync(
    db: &Arc<ClipboardDb>,
    settings: &mut Settings,
) -> Result<(), String> {
    let db = Arc::clone(db);
    let mut next = settings.clone();
    next.webdav_last_sync_at = Some(Utc::now().to_rfc3339());
    let next = super::spawn_block(
        "WebDAV 保存设置",
        move || -> Result<Settings, String> {
            db.save_sync_metadata(&next).map_err(|e| e.to_string())
        },
    )
    .await?;
    *settings = next;
    Ok(())
}

pub(super) fn client_from_settings(settings: &Settings) -> Result<WebDavClient, String> {
    if settings.webdav_url.trim().is_empty() {
        return Err("请先填写 WebDAV 服务器地址".into());
    }
    if settings.webdav_username.trim().is_empty() {
        return Err("请先填写 WebDAV 用户名".into());
    }
    WebDavClient::new(
        &settings.webdav_url,
        &settings.webdav_username,
        &settings.webdav_password,
    )
}

pub(super) async fn fetch_remote_state(
    client: &WebDavClient,
    root: &str,
    device_id: &str,
) -> Result<RemoteState, String> {
    let manifest_rel = join_remote(root, MANIFEST_NAME);
    let manifest_remote = client.get_bytes_with_etag(&manifest_rel).await?;
    let (manifest, manifest_etag, manifest_exists) = match manifest_remote {
        Some(remote) => (
            serde_json::from_slice::<SyncManifest>(&remote.bytes)
                .map_err(|e| format!("解析 manifest.json 失败: {e}"))?,
            remote.etag,
            true,
        ),
        None => (SyncManifest::empty(device_id), None, false),
    };

    let bundle_rel = join_remote(root, BUNDLE_REL);
    let bundle_remote = client.get_bytes_with_etag(&bundle_rel).await?;
    let (records, bundle_etag, bundle_exists, bundle_bytes) = match bundle_remote {
        Some(remote) => {
            let bytes = remote.bytes;
            // Bundle parse is pure CPU over up-to-64MB — run off the async worker.
            let (records, bytes) =
                super::spawn_block("解析 bundle", move || -> Result<_, String> {
                    let records = parse_bundle(&bytes)?;
                    Ok((records, bytes))
                })
                .await?;
            (records, remote.etag, true, Some(bytes))
        }
        None => (Vec::new(), None, false, None),
    };
    Ok(RemoteState {
        manifest,
        records,
        manifest_etag,
        manifest_exists,
        bundle_etag,
        bundle_exists,
        bundle_bytes,
    })
}

pub(super) struct RemoteState {
    pub manifest: SyncManifest,
    pub records: Vec<crate::ClipboardRecord>,
    pub manifest_etag: Option<String>,
    pub manifest_exists: bool,
    pub bundle_etag: Option<String>,
    pub bundle_exists: bool,
    pub bundle_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WebDavSyncResult {
    pub pulled: i32,
    pub pushed: i32,
    pub merged: i32,
    pub tags_pulled: i32,
    pub tags_pushed: i32,
    pub media_downloaded: i32,
    pub media_uploaded: i32,
    pub media_skipped: i32,
}
