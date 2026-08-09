//! Commands for the OS clipboard-history import flow.
//!
//! The startup import itself is fire-and-forget from the first window `Focused`
//! event (`win_history`). It emits `clipboard-history-imported` to live
//! listeners, but that event can be lost when the import completes before the
//! webview finished registering its listeners on boot. `get_pending_history_import`
//! is the frontend's one-shot catch-up for that window.

use crate::win_history;

/// Read-and-reset the inserted count of the most recent startup history import.
/// Returns `None` when there is nothing pending (no run finished, or the event
/// was already delivered live).
#[tauri::command]
pub fn get_pending_history_import() -> Option<usize> {
    let count = win_history::take_pending_import();
    (count > 0).then_some(count)
}
