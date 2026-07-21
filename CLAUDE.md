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
- **Frontend:** Vue 3 + TypeScript + Vite + Pinia; Lucide icons; DOMPurify for rich-text preview
- **Backend:** Rust (Tauri v2 plugins: single-instance, clipboard, dialog, FS, global-shortcut, shell, SQL, autostart)
- **Database:** SQLite via rusqlite (WAL), `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media:** PNG + JPEG thumbs under `%LOCALAPPDATA%/ClipVault/media/`; DB stores paths/size only
- **Clipboard polling:** arboard every 500ms, but **`GetClipboardSequenceNumber` skips all reads** when OS clipboard unchanged (avoids per-tick `get_image` RGBA). Clipboard handle is reused. Image path moves owned buffers into `store_clipboard_image` (no double `to_vec`).
- **List IPC:** `substr(content,1,400)` + `content_len`; `content_html` omitted. `clipboard-changed` emits the same light payload. Detail/`get_record` still full.
- **Stats:** `media/` directory size cached ~30s. Frontend `loadStats` debounced 800ms on new records.
- **Expire sweep:** watches `auto_expire_at` list (not deep `records`).
- **Appearance IPC:** `set_window_corner_radius` only when `panel_radius` changes.
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
├── FloatingPanel.vue            # Floating UI; filters; empty trash; useBatchActions + useClipboardHotkeys
├── WindowApp.vue                # Window UI; SideBar; same batch/hotkeys helpers
│   ├── SearchBar.vue
│   ├── RecordList.vue           # Infinite scroll; thumbs; PreviewPane
│   │   └── PreviewPane.vue      # Header meta line; DOMPurify HTML preview; paste / tags / trash
│   └── SideBar.vue              # Categories (+ favorites); trash; tags; capture/theme/settings icons
├── SettingsWindow.vue           # Nav: 外观 → 快捷键 → 历史 → 隐私 → 系统 → 数据 → 统计 → 关于
├── WindowControls.vue
├── ToastHost.vue
├── ConfirmDialog.vue / TagDialog.vue
├── composables/useBatchActions.ts · useClipboardHotkeys.ts · useToast.ts · useConfirm.ts
├── utils/mediaUrl.ts · sanitizeHtml.ts
└── TrayMenu.vue                 # Placeholder (native tray is Rust)
```

### Backend (Rust) Module Layout
- `lib.rs` — setup, commands, tray, shortcuts, autostart, sensitive detection, cleanup throttle, **window round corners** (`CreateRoundRectRgn` / `SetWindowRgn`, synced on resize / settings / mode switch)
- `clipboard.rs` — monitor + paste via `keybd_event`
- `media.rs` — encode/store/load/delete
- `db.rs` — CRUD, FTS5, trash, tags, stats, settings
- `main.rs` — `clipvault_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, category×tag AND filters, trash exclusive, batch, pause, pagination (60 / `has_more`), `ensureRecordDetail` for HTML
- `settingsStore` — debounced auto-save (200ms); theme / appearance (`font_size`, `panel_radius`, `panel_opacity`, blur); applies CSS vars + `set_window_corner_radius`

### Key Design Decisions
- **Floating vs window:** Both borderless, `transparent: true`, `shadow: false`. Floating: always-on-top, hide on blur. Window: SideBar + `WindowControls`. Shared `.panel-surface` chrome. **Size:** `resolve_panel_size` prefers last user resize (`floating_*` / `window_*` in settings); if unset (0), falls back to `adaptive_panel_size` (floating ≈ 40%×65%, window ≈ 55%×72%, clamped). Resize is debounced ~400ms into SQLite; maximized sizes are not saved. Frontend `save_settings` never overwrites size fields.
- **True round corners (Windows):** CSS `border-radius` alone leaves black rectangular corners on transparent WebView2. Clip HWND with `SetWindowRgn` from `panel_radius` × DPI. Command: `set_window_corner_radius`.
- **Theming:** CSS vars on `:root`; `.light-theme` / `.oled-theme` via `document.body.classList`. SideBar can toggle dark↔light.
- **Font size:** Root `font-size` = setting (default **16px**). Rem baseline is **16px** (`--ui-font-scale = font_size/16`). Main UI (list / preview / sidebar / floating chrome) uses `rem`; Settings page keeps fixed `px` so the settings UI does not jump while dragging the slider.
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s.
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Memory (frontend):** List soft-capped (`PAGE_SIZE * 2`) on `onNewRecord`. Full content/HTML live in a small `recordDetails` map (max ~6), never merged into list rows. `loadRecords`/search clears detail cache.
- **Clipboard fingerprint:** SHA-256 of text+html (not retaining full HTML string in `last_text_fp`).
- **Retention “回收站保留天数”:** Only purges trashed rows (not the whole history). **最大记录数** evicts oldest non-favorite / non-pinned when inserting.
- **Toast policy:** Actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML; preview loads via `get_record` / detail. Preview uses **DOMPurify + `v-html`** (not iframe) when markup differs from plain. Display CSS may force wrap; stored HTML for paste is unchanged. Manual select-copy from preview may normalize whitespace.
- **Preview chrome:** Type + actions in header; source / time / size-or-chars / 富文本 as one meta line (`title` = content type). Single scroll on `.preview-content` (no nested scroll on content / rich HTML).
- **Filters:** Type/favorites **AND** tag combine; trash is exclusive. IPC: `get_records` / `search_records` / `get_all_tags` use `#[tauri::command(rename_all = "snake_case")]` — pass `content_type`, `favorites_only`, `tag`, `trashed`. Tag counts follow active category (`get_all_tags`).
- **Search:** FTS5 trigram (≥3 chars) on content / source_app / source_window / tags; shorter queries use escaped `LIKE`. FTS sync via triggers. **FTS v2:** use `DELETE FROM records_fts WHERE rowid=…` in triggers — the FTS5 `'delete'` command returns `SQL logic error` on current Windows SQLite and breaks empty-trash / permanent delete.
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set`.
- **Global shortcut:** From `settings.global_shortcut` at startup; re-bound in `save_settings`.
- **Pause capture:** Frontend + tray both update Rust; tray emits `capture-paused`.
- **Cleanup:** Expired/retention throttled (~60s).
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint or full image bytes.
- **Hide-on-close / single instance / autostart:** tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles map 1:1 to tracker labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.
