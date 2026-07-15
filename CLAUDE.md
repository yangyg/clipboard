# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm run dev          # Start Vite dev server (port 1420)
npm run build        # vue-tsc type-check + vite build
npm run preview      # Preview the built frontend
npm run tauri        # Run Tauri CLI commands (e.g., npm run tauri dev)
```

The full Tauri dev command is `npm run tauri dev` (starts both Vite + Rust backend).

## Architecture

ClipVault is a **Tauri v2** desktop clipboard manager for Windows.

### Stack
- **Frontend:** Vue 3 + TypeScript + Vite + Pinia (state management)
- **Backend:** Rust (Tauri v2 with plugins for clipboard, dialog, FS, global-shortcut, shell, SQL, autostart)
- **Database:** SQLite via rusqlite (WAL mode), stored at `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Clipboard polling:** arboard crate polls clipboard text every 500ms on a background thread
- **Autostart:** `tauri-plugin-autostart` (=2.2.0) registers Windows startup via the OS (Run key / equivalent). Controlled only from Rust (`save_settings` / app setup); no frontend JS plugin binding.

### Data Flow
1. Rust `ClipboardMonitor` polls the OS clipboard every 500ms
2. On change: hash content, detect type (text/code/link/file) and sensitivity, insert into SQLite
3. Emit `clipboard-changed` event to Vue frontend via Tauri events
4. Vue `App.vue` listens and updates the Pinia store
5. User can paste via simulated Ctrl+V (`keybd_event` on Windows), which replaces clipboard text with the stored record and sends the keystroke to the foreground window

### Frontend Component Tree
```
App.vue                          # Root: listens for Tauri events, manages show/hide
├── FloatingPanel.vue            # Main panel: header, filter tabs, stats, batch bar
│   ├── SearchBar.vue            # Debounced search (150ms), / or Ctrl+K to focus
│   └── RecordList.vue           # Left sidebar list + right preview pane
│       └── PreviewPane.vue      # Detail view per content type (text/code/link/image/file)
│           └── ActionBar.vue    # Paste, plain-text paste, favorite, pin, delete
├── SettingsWindow.vue           # Full settings UI with nav sidebar
└── TrayMenu.vue                 # Placeholder (native tray is Rust-rendered)
```

### Backend (Rust) Module Layout
- `src-tauri/src/lib.rs` — App setup, Tauri commands, system tray, global shortcut, content detection, sensitive detection, autostart sync (`apply_autostart`)
- `src-tauri/src/clipboard.rs` — `ClipboardMonitor` (polling loop), paste simulation via `keybd_event` (Windows)
- `src-tauri/src/db.rs` — `ClipboardDb`: records CRUD, search, settings persistence, import/export, stats
- `src-tauri/src/main.rs` — Entry point, calls `clipvault_lib::run()`

### State Management (Pinia Stores)
- `clipboardStore` — records array, selection, search, filters (all/text/code/link/image/file/favorites), batch mode, pause capture, stats
- `settingsStore` — all app settings with auto-save on change (debounced 200ms via `watch`), theme application. Changing `auto_start` persists via `save_settings`, which enables/disables OS autostart on the Rust side first; on failure the UI reloads settings so the toggle stays consistent.

### Key Design Decisions
- **Floating mode** (default): borderless always-on-top window that auto-hides on focus loss. Window mode: standard decorated window.
- **Theming**: CSS custom properties on `:root` (dark default), class-based overrides (`.light-theme`, `.oled-theme`). Applied via `document.body.classList`.
- **Sensitive content detection**: regex patterns for passwords, verification codes, API keys (`sk-...`), bank card numbers. Marked records auto-expire after configurable seconds.
- **Clipboard paste**: sets clipboard content via arboard, then simulates Ctrl+V. No IPC to foreground app.
- **Search**: SQL `LIKE` on content and source_app. Debounced 150ms frontend side.
- **Deduplication**: by SHA-256 content hash. Same hash = increment copy count + update timestamp, no new record.
- **Window hide-on-close**: `CloseRequested` event calls `api.prevent_close()` and hides window to minimize to tray.
- **Autostart**: `settings.auto_start` (default `false`) is not UI-only — `save_settings` applies OS registration before persisting, and app `setup` re-syncs from loaded settings (skips sync if settings fail to load). OS failures surface as `save_settings` errors; DB save failure after a successful OS change reverts the startup entry.
