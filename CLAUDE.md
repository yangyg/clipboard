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
- **Backend:** Rust (Tauri v2 with plugins for single-instance, clipboard, dialog, FS, global-shortcut, shell, SQL, autostart)
- **Database:** SQLite via rusqlite (WAL mode), stored at `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media files:** Images stored as PNG under `%LOCALAPPDATA%/ClipVault/media/` (+ `thumbs/`); DB holds paths/size only
- **Clipboard polling:** arboard crate polls clipboard text/images every 500ms on a background thread
- **Autostart:** `tauri-plugin-autostart` (=2.2.0) registers Windows startup via the OS (Run key / equivalent). Controlled only from Rust (`save_settings` / app setup); no frontend JS plugin binding.

### Data Flow
1. Rust `ClipboardMonitor` polls the OS clipboard every 500ms
2. On change: hash content; text → SQLite `content`; image → write `media/` + thumb, DB stores metadata only
3. Emit `clipboard-changed` event to Vue frontend via Tauri events
4. Vue `App.vue` listens and updates the Pinia store
5. Paste branches by type: text → `set_text` + Ctrl+V; image → `set_image` from disk + Ctrl+V

### Frontend Component Tree
```
App.vue                          # Root: events, show/hide, ToastHost + ConfirmDialog
├── FloatingPanel.vue            # Floating: search-first, filters, trash, batch; shared hotkeys
├── WindowApp.vue                # Window: SideBar + list + batch; shared hotkeys
│   ├── SearchBar.vue            # Debounced search (150ms), / or Ctrl+K (platform-aware)
│   ├── RecordList.vue           # List + PreviewPane; scroll-into-view; context menu
│   │   └── PreviewPane.vue      # Preview + paste/favorite/pin/delete + tags
│   └── SideBar.vue              # Categories, tags, trash (window mode)
├── SettingsWindow.vue           # Header + nav + body; shortcut recording; behavior toggles
├── ToastHost.vue / ConfirmDialog.vue
├── icons/AppIcon.vue · TypeIcon.vue · BrandMark.vue  # Lucide linear icons + brand mark
└── TrayMenu.vue                 # Placeholder (native tray is Rust-rendered)
```

### Backend (Rust) Module Layout
- `src-tauri/src/lib.rs` — App setup, Tauri commands, system tray, global shortcut, content detection, sensitive detection, autostart sync (`apply_autostart`)
- `src-tauri/src/clipboard.rs` — `ClipboardMonitor` (polling loop), paste text/image via `keybd_event` (Windows)
- `src-tauri/src/media.rs` — Image encode/store/load/delete under app data `media/`
- `src-tauri/src/db.rs` — `ClipboardDb`: records CRUD, media lifecycle on hard-delete, settings, import/export, stats
- `src-tauri/src/main.rs` — Entry point, calls `clipvault_lib::run()`

### State Management (Pinia Stores)
- `clipboardStore` — records array, selection, search, filters (all/text/code/link/image/file/favorites), batch mode, pause capture, stats
- `settingsStore` — all app settings with auto-save on change (debounced 200ms via `watch`), theme application. Changing `auto_start` persists via `save_settings`, which enables/disables OS autostart on the Rust side first; on failure the UI reloads settings so the toggle stays consistent.

### Key Design Decisions
- **Floating mode** (default): borderless always-on-top window that auto-hides on focus loss. Window mode: standard decorated window.
- **Theming**: CSS custom properties on `:root` (dark default), class-based overrides (`.light-theme`, `.oled-theme`). Applied via `document.body.classList`.
- **Sensitive content detection**: regex patterns for passwords, verification codes, API keys (`sk-...`), bank card numbers. Marked records auto-expire after configurable seconds.
- **Clipboard paste**: by `content_type` — text uses `set_html`+plain alt when `content_html` exists (原格式) or `set_text` only (纯文本); image loads PNG from disk and uses `set_image`; then simulates Ctrl+V. No IPC to foreground app.
- **Rich text**: capture reads CF_HTML via arboard `get().html()` into `content_html`; list/search still use plain `content`.
- **Image storage**: SQLite does not store image blobs; binary lives in `media/` with JPEG thumbs for the list.
- **Search**: SQL `LIKE` on content and source_app. Debounced 150ms frontend side.
- **Deduplication**: by SHA-256 content hash. Same hash = increment copy count + update timestamp, no new record.
- **Window hide-on-close**: `CloseRequested` event calls `api.prevent_close()` and hides window to minimize to tray.
- **Single instance**: `tauri-plugin-single-instance` (registered first) ensures only one process runs. A second launch focuses the existing window instead of competing for `Ctrl+Shift+V` / clipboard monitor.
- **Autostart**: `settings.auto_start` (default `false`) is not UI-only — `save_settings` applies OS registration before persisting, and app `setup` re-syncs from loaded settings (skips sync if settings fail to load). Sync is idempotent: already-on/off is a no-op (Windows `disable` errors if the Run value is missing). OS failures surface as `save_settings` errors; DB save failure after a successful OS change reverts the startup entry.
