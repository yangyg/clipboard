//! WebDAV sync: pull/merge/push of manifest + JSONL records + hashed media.
//! Capture hot path never calls into this module.

mod bundle;
mod client;
mod media;
mod sync;
mod tags;

pub use sync::{webdav_pull, webdav_push, webdav_sync, webdav_test_connection, WebDavSyncResult};

/// Run a blocking DB/fs operation on the tokio blocking pool, with the
/// standard `"WebDAV/媒体 …任务失败: {e}"` error wrapping. Every async sync
/// step used to hand-roll this same `.spawn_blocking().await.map_err()` pair.
/// `label` supplies the prefix (e.g. `"WebDAV 加载本地记录"`, `"媒体读取"`).
async fn spawn_block<T, E>(
    label: &'static str,
    f: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("{label}任务失败: {e}"))?
        .map_err(|e| e.to_string())
}
