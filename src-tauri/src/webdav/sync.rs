//! Pull / merge / push orchestration for Clipboard WebDAV sync.
//! Stage implementations live in `sync_common` / `sync_merge` / `sync_pull` / `sync_push`.

use std::sync::Arc;

use crate::db::ClipboardDb;
use crate::Settings;

use super::sync_common::client_from_settings;

pub use super::sync_common::WebDavSyncResult;
pub use super::sync_pull::webdav_pull;
pub use super::sync_push::webdav_push;

pub async fn webdav_test_connection(settings: &Settings) -> Result<(), String> {
    let client = client_from_settings(settings)?;
    client.test_connection().await
}

pub async fn webdav_sync(
    db: &Arc<ClipboardDb>,
    settings: &mut Settings,
) -> Result<WebDavSyncResult, String> {
    let pull = webdav_pull(db, settings).await?;
    let push = webdav_push(db, settings).await?;
    Ok(WebDavSyncResult {
        pulled: pull.pulled,
        pushed: push.pushed,
        merged: pull.merged,
        tags_pulled: pull.tags_pulled,
        tags_pushed: push.tags_pushed,
        media_downloaded: pull.media_downloaded,
        media_uploaded: push.media_uploaded,
        media_skipped: push.media_skipped,
    })
}
