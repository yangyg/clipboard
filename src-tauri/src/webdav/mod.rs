//! WebDAV sync: pull/merge/push of manifest + JSONL records + hashed media.
//! Capture hot path never calls into this module.

mod bundle;
mod client;
mod media;
mod sync;

pub use sync::{
    webdav_pull, webdav_push, webdav_sync, webdav_test_connection, WebDavSyncResult,
};
