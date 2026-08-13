//! All `#[tauri::command]` handlers, split by feature area into submodules.
//! `pub use …::*` re-exports keep the `commands::*` paths used by
//! `invoke_handler` in lib.rs unchanged.

mod ai;
mod fonts;
mod history;
mod import_export;
mod paste;
mod records;
mod search_history;
mod settings;
mod tags;
mod tray;
mod webdav;

pub use ai::*;
pub use fonts::*;
pub use history::*;
pub use import_export::*;
pub use paste::*;
pub use records::*;
pub use search_history::*;
pub use settings::*;
pub use tags::*;
pub use tray::*;
pub use webdav::*;

use crate::AppState;
use crate::FeatureId;
use tauri::State;

pub(crate) fn settings_features(
    state: &State<'_, AppState>,
) -> Result<crate::FeatureFlags, String> {
    let s = state.db.get_settings().map_err(|e| e.to_string())?;
    Ok(s.features.clone())
}

/// Gate a command on a product capability, resolving the settings from the
/// store in one call. Mirrors `require_feature` but for the common
/// `state.db.get_settings()` + `FeatureId::X` pair that every command repeats.
pub(crate) fn require_feature_state(
    state: &State<'_, AppState>,
    id: FeatureId,
) -> Result<(), String> {
    let settings = state.db.get_settings().map_err(|e| e.to_string())?;
    crate::require_feature(&settings, id)
}

/// Upper bound for page-size IPC args — a compromised webview must not be able
/// to materialize every record (incl. sensitive) in a single call.
///
/// Distinct from `db::MAX_PAGE_SIZE` (500, the DB-level cap): commands clamp
/// user input to this stricter IPC bound so the webview can never request the
/// full table in one page.
pub(crate) const MAX_IPC_PAGE_SIZE: i32 = 200;
/// Upper bound for batch id args, keeps placeholders / SQL bounded.
pub(crate) const MAX_BATCH_IDS: usize = 1000;

pub(crate) fn cap_ids(ids: Vec<i64>) -> Vec<i64> {
    ids.into_iter().take(MAX_BATCH_IDS).collect()
}

/// Run a blocking database operation on the tokio blocking pool.
///
/// Tauri async commands otherwise execute their body on async worker threads;
/// rusqlite calls are blocking, so a slow query (or a capture insert holding
/// the write lock) would occupy an async worker and stall unrelated IPC.
/// `spawn_blocking` moves the DB work to the dedicated blocking pool while the
/// command future just awaits the join handle.
pub(crate) async fn spawn_db<F, T, E>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("database task failed: {e}"))?
        .map_err(|e| e.to_string())
}
