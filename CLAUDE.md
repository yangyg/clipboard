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

Regenerate app icons from a source image (PNG preferred; JPEG renamed as `.png` must be converted first):

```bash
npx tauri icon app-icon.png -o src-tauri/icons
```

## Architecture

ClipVault is a **Tauri v2** desktop clipboard manager for Windows.

### Stack
- **Frontend:** Vue 3 + TypeScript + Vite + Pinia
- **Backend:** Rust (Tauri v2 plugins: single-instance, clipboard, dialog, FS, global-shortcut, shell, SQL, autostart)
- **Database:** SQLite via rusqlite (WAL), `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media:** PNG + JPEG thumbs under `%LOCALAPPDATA%/ClipVault/media/`; DB stores paths/size only
- **Clipboard polling:** arboard every 500ms; image quick-fingerprint before full SHA-256; text/image priority heuristics
- **Asset protocol:** `protocol-asset`; scope uses `$LOCALDATA/ClipVault/media/**/*` (not `$LOCALAPPDATA`)
- **Autostart / shortcut / ignore list:** applied from Rust on `save_settings` / setup (not frontend-only)

### Data Flow
1. `ClipboardMonitor` polls every 500ms; skip work when image quick-fp unchanged
2. **Image vs text:** Prefer text only for meaningful shares (≥16 chars, not URL-only). Screenshots / browser “Copy image” (URL-only text) → image
3. Skip capture when `source_app` matches `settings.ignored_apps`
4. Persist: text (+ optional `content_html`) → SQLite; image → `media/` + thumb + metadata label `[image WxH]`
5. Emit `clipboard-changed`; Vue store updates list
6. Paste: text → `set_html`/`set_text` + Ctrl+V; image → disk PNG → `set_image` + Ctrl+V

### Frontend Component Tree
```
App.vue                          # Events (clipboard-changed, capture-paused, toggle-panel), ToastHost, ConfirmDialog
├── FloatingPanel.vue            # Floating UI; filters; trash; useBatchActions + useClipboardHotkeys
├── WindowApp.vue                # Window UI; SideBar; same batch/hotkeys helpers
│   ├── SearchBar.vue
│   ├── RecordList.vue           # Infinite scroll; thumbs precomputed; PreviewPane
│   │   └── PreviewPane.vue      # Paste / favorite / pin / trash; tags; expire countdown
│   └── SideBar.vue              # Categories/tags as <button>; tag toggle-off; context edit/delete
├── SettingsWindow.vue
├── WindowControls.vue
├── ToastHost.vue                # Top-center; error → aria-live assertive
├── ConfirmDialog.vue / TagDialog.vue
├── composables/useBatchActions.ts · useClipboardHotkeys.ts · useToast.ts · useConfirm.ts
├── utils/mediaUrl.ts
└── TrayMenu.vue                 # Placeholder (native tray is Rust)
```

### Backend (Rust) Module Layout
- `lib.rs` — setup, commands, tray, `apply_global_shortcut`, `apply_autostart`, `ignored_apps`, content/sensitive detection, cleanup throttle (`maybe_run_cleanup` ~60s)
- `clipboard.rs` — monitor (quick image fp + share-text heuristics), paste via `keybd_event`
- `media.rs` — encode/store/load/delete; segment-joined absolute paths
- `db.rs` — CRUD, list cols without `content_html`, search with type/tag/favorites filters, tags, stats (DB + media dir size)
- `main.rs` — `clipvault_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, filters, tags, batch (`selectedIds` replaced as new `Set` for reactivity), pause, pagination (60 / `has_more`), `ensureRecordDetail` for HTML, `setPauseCapture` for tray sync
- `settingsStore` — debounced auto-save (200ms); `auto_start` / shortcut / appearance; failed saves reload UI

### Key Design Decisions
- **Floating vs window:** Both borderless. Floating: always-on-top, hide on blur. Window: SideBar + `WindowControls`.
- **Theming:** CSS vars on `:root`; `.light-theme` / `.oled-theme` via `document.body.classList`.
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s.
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Toast policy:** Only for actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles. Position: top-center.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML (`NULL as content_html`); preview loads via `get_record`. Show HTML iframe only when markup differs from plain.
- **Image storage + asset scope:** See stack; wrong `$VAR` → asset protocol 403.
- **Pagination / search:** Server-side filters only. Search uses FTS5 trigram (≥3 chars) over content/source_app/source_window/tags; shorter queries use escaped `LIKE`. Args: `contentType` / `favoritesOnly` / `tag`.
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set` (`selectedIds`, `assignedIds`).
- **Global shortcut:** Registered from `settings.global_shortcut` at startup; re-bound in `save_settings` when changed.
- **Pause capture:** Frontend `set_capture_paused` and tray both update Rust; tray emits `capture-paused` for UI sync.
- **Cleanup:** Expired/retention cleanup throttled (~60s), not on every list/stats call.
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint or full image bytes.
- **Hide-on-close / single instance / autostart:** unchanged tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.
