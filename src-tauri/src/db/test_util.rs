//! Shared DB test scaffolding: temp-dir ClipboardDb + cleanup.
//! Every `#[cfg(test)]` module in `db/` used to copy its own `temp_db()`
//! (unique prefix, same body) and `cleanup()`. A signature change to
//! `ClipboardDb::new` or the temp layout previously meant editing 7 copies.

use super::ClipboardDb;
use std::path::PathBuf;

/// Create a fresh `ClipboardDb` in a unique temp dir. `label` becomes part of
/// the directory name so concurrent test binaries never collide.
pub fn temp_db(label: &str) -> (ClipboardDb, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "clipvault_{label}_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = ClipboardDb::new(&dir.join("test.db"), dir.clone()).unwrap();
    (db, dir)
}

/// Best-effort teardown: drop the SQLite files then the whole temp dir.
/// Idempotent — safe to call even after a failed test.
pub fn cleanup(dir: PathBuf) {
    for name in ["test.db", "test.db-wal", "test.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let _ = std::fs::remove_dir_all(dir);
}
