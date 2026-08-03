//! All `#[tauri::command]` handlers, split by feature area into submodules.
//! `pub use …::*` re-exports keep the `commands::*` paths used by
//! `invoke_handler` in lib.rs unchanged.

mod import_export;
mod paste;
mod records;
mod settings;
mod tags;
mod tray;
mod webdav;

pub use import_export::*;
pub use paste::*;
pub use records::*;
pub use settings::*;
pub use tags::*;
pub use tray::*;
pub use webdav::*;

use tauri::State;
use crate::AppState;

pub(crate) fn settings_features(state: &State<'_, AppState>) -> Result<crate::FeatureFlags, String> {
    let s = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok(s.features.clone())
}

/// Upper bound for page-size IPC args — a compromised webview must not be able
/// to materialize every record (incl. sensitive) in a single call.
pub(crate) const MAX_PAGE_SIZE: i32 = 200;
/// Upper bound for batch id args, keeps placeholders / SQL bounded.
pub(crate) const MAX_BATCH_IDS: usize = 1000;

pub(crate) fn cap_ids(ids: Vec<i64>) -> Vec<i64> {
    ids.into_iter().take(MAX_BATCH_IDS).collect()
}
