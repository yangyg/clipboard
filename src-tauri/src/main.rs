// ============================================================
// ClipVault — Rust Backend Entry Point
// ============================================================

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clipvault_lib::run;

fn main() {
    run();
}
