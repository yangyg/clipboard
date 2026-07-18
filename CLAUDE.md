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
- **Media files:** Images stored as PNG under `%LOCALAPPDATA%/ClipVault/media/` (+ `thumbs/` JPEG); DB holds relative paths + width/height only
- **Clipboard polling:** arboard polls every 500ms on a background thread (**image before text**)
- **Asset protocol:** `protocol-asset` enabled; scope must use Tauri vars (`$LOCALDATA`, not OS env names like `$LOCALAPPDATA`)
- **Autostart:** `tauri-plugin-autostart` registers Windows startup via the OS. Controlled only from Rust (`save_settings` / app setup); no frontend JS plugin binding.

### Data Flow
1. Rust `ClipboardMonitor` polls the OS clipboard every 500ms
2. **Prefer image:** `get_image()` first. Windows screenshots / browser copies often also set CF_TEXT/HTML; text-first would skip images. When an image is captured, also sync the text fingerprint so accompanying text is not treated as a new record on the next poll.
3. On change: hash content; text (+ optional `content_html`) → SQLite; image → write `media/` + thumb, DB stores metadata + label like `[image WxH]`
4. Emit `clipboard-changed` to Vue; store prepends / updates the list
5. Paste by type: text → `set_html`/`set_text` + Ctrl+V; image → load PNG from disk → `set_image` + Ctrl+V

### Frontend Component Tree
```
App.vue                          # Root: events, show/hide, ToastHost + ConfirmDialog
├── FloatingPanel.vue            # Floating: search-first, filters, trash, batch; shared hotkeys
├── WindowApp.vue                # Window: SideBar + list + batch; WindowControls; shared hotkeys
│   ├── SearchBar.vue            # Debounced search (150ms), / or Ctrl+K (platform-aware)
│   ├── RecordList.vue           # List + PreviewPane; infinite scroll; context menu
│   │   └── PreviewPane.vue      # Preview + paste/favorite/pin/delete + tags
│   └── SideBar.vue              # Categories, tags (edit/delete via context menu), trash
├── SettingsWindow.vue           # Header + nav + body; shortcut recording; behavior toggles
├── WindowControls.vue           # Custom min/max/close (borderless chrome)
├── ToastHost.vue / ConfirmDialog.vue / TagDialog.vue
├── utils/mediaUrl.ts            # convertFileSrc for media_abs / thumb_abs
├── icons/AppIcon.vue · TypeIcon.vue · BrandMark.vue
└── TrayMenu.vue                 # Placeholder (native tray is Rust-rendered)
```

### Backend (Rust) Module Layout
- `src-tauri/src/lib.rs` — App setup, Tauri commands, system tray, global shortcut, content detection, sensitive detection, autostart sync (`apply_autostart`)
- `src-tauri/src/clipboard.rs` — `ClipboardMonitor` (image-first poll), paste text/image via `keybd_event` (Windows)
- `src-tauri/src/media.rs` — Image encode/store/load/delete; absolute paths built segment-by-segment (avoid mixed `/` `\`)
- `src-tauri/src/db.rs` — `ClipboardDb`: records CRUD, pagination, media lifecycle on hard-delete, settings, import/export, stats, tags
- `src-tauri/src/main.rs` — Entry point, calls `clipvault_lib::run()`

### State Management (Pinia Stores)
- `clipboardStore` — records array, selection, search, filters (all/text/code/link/image/file/favorites), tags, batch mode, pause capture, stats; scroll pagination (page size 60, `RecordsPage { records, has_more }`)
- `settingsStore` — all app settings with auto-save on change (debounced 200ms via `watch`), theme application. Changing `auto_start` persists via `save_settings`, which enables/disables OS autostart on the Rust side first; on failure the UI reloads settings so the toggle stays consistent.

### Key Design Decisions
- **Floating vs window mode:** Both are borderless. Floating: always-on-top, auto-hide on focus loss. Window: larger layout with SideBar + custom `WindowControls`.
- **Theming:** CSS custom properties on `:root` (dark default), class-based overrides (`.light-theme`, `.oled-theme`). Applied via `document.body.classList`.
- **Sensitive content detection** (`detect_sensitive` on text only; images always non-sensitive). Enabled by `enable_sensitive_detection` (default on). Auto-expire via `sensitive_auto_expire_seconds` (default 600). Rules (any match):
  - Contains `password` / `passwd` / `pwd` (case-insensitive)
  - 4–8 digit run **and** contains `验证码` / `code` / `Code`
  - API key `sk-` + ≥20 alphanumeric
  - 16–19 digit run **and** whole string length ≤ 25
- **Clipboard paste:** by `content_type` — text uses `set_html`+plain alt when `content_html` exists (原格式) or `set_text` only (纯文本); image loads PNG from disk and uses `set_image`; then simulates Ctrl+V. No IPC to foreground app.
- **Rich text:** capture reads CF_HTML via arboard `get().html()` into `content_html`; list/search still use plain `content`.
- **Image storage:** SQLite does not store image blobs; binary in `media/`, JPEG thumbs in `media/thumbs/`. Frontend loads via `convertFileSrc(media_abs|thumb_abs)`.
- **Asset protocol scope:** `tauri.conf.json` → `app.security.assetProtocol.scope`: `["$LOCALDATA/ClipVault/media/**/*"]`. Wrong variable names (e.g. `$LOCALAPPDATA`) silently fail matching → “asset protocol not configured to allow the path”.
- **Pagination:** `get_records` / `search_records` take `limit`/`offset` (default 60); return `has_more`. Sidebar counts come from `stats`, not the loaded page.
- **Search:** SQL `LIKE` on content and source_app. Debounced 150ms frontend side.
- **Deduplication:** by SHA-256 content hash. Same hash = increment copy count + update timestamp, no new record.
- **Window hide-on-close:** `CloseRequested` event calls `api.prevent_close()` and hides window to minimize to tray.
- **Single instance:** `tauri-plugin-single-instance` (registered first) ensures only one process runs. A second launch focuses the existing window instead of competing for the hotkey / clipboard monitor.
- **Autostart:** `settings.auto_start` (default `false`) is not UI-only — `save_settings` applies OS registration before persisting, and app `setup` re-syncs from loaded settings (skips sync if settings fail to load). Sync is idempotent. OS failures surface as `save_settings` errors; DB save failure after a successful OS change reverts the startup entry.
- **WebView noise:** `Failed to unregister class Chrome_WidgetWin_0. Error = 1412` on exit/hot-reload is harmless Chromium/WebView2 teardown, not an app bug.
