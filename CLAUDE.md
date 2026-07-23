# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm run dev          # Start Vite dev server (port 1420)
npm run build        # vue-tsc type-check + vite build
npm run preview      # Preview the built frontend
npm run tauri        # Run Tauri CLI commands (e.g., npm run tauri dev)
npm test             # Run Vitest once (Pinia store smoke tests, jsdom)
npm run lint         # Run ESLint over src (.ts + .vue)

cargo test --manifest-path src-tauri/Cargo.toml   # Run Rust backend tests (17 tests)
```

The full Tauri dev command is `npm run tauri dev` (starts both Vite + Rust backend).

**After modifying Rust code** (`src-tauri/src/*.rs`), run `cargo test --manifest-path src-tauri/Cargo.toml` to verify the backend still passes its tests.

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
- **Clipboard polling:** arboard every 500ms, but **`GetClipboardSequenceNumber` skips all reads** when OS clipboard unchanged. **Text-first:** meaningful share text skips `get_image()`; otherwise only call it when `IsClipboardFormatAvailable` reports bitmap/DIB. Monitor **enqueues** to a bounded worker (`sync_channel(4)`) so PNG encode / SQLite / auto-tag do not block the poll thread.
- **List IPC:** `substr(content,1,400)` + `content_len`; `content_html` omitted. `clipboard-changed` emits the same light payload. Detail/`get_record` still full.
- **List UI:** `RecordList` window-virtualizes rows (fixed-height estimate + overscan); soft-cap still bounds in-memory pages. Export streams JSON to a user-chosen path (no full `Vec` / string in IPC). Preview HTML sanitize is fingerprint-cached.
- **Stats:** `media/` size cached 120s and **incrementally adjusted** on image store/delete (no re-walk until TTL). Frontend `loadStats` debounced 800ms. Tag assign uses `set_record_tags` (one transaction). Non-default list sorts debounce reload (~400ms) on new captures.
- **Expire sweep:** watches `auto_expire_at` list (not deep `records`).
- **Appearance IPC:** `set_window_corner_radius` only when `panel_radius` changes.
- **Asset protocol:** `protocol-asset`; scope uses `$LOCALDATA/ClipVault/media/**/*` (not `$LOCALAPPDATA`)
- **Autostart / shortcut / ignore list:** applied from Rust on `save_settings` / setup (not frontend-only)

### Data Flow
1. `ClipboardMonitor` polls every 500ms; skip work when image quick-fp unchanged
2. **Image vs text:** Prefer text only for meaningful shares (≥16 chars, not URL-only). Screenshots / browser “Copy image” (URL-only text) → image
3. Skip capture when `source_app` matches `settings.ignored_apps`
4. Persist: text (+ optional `content_html`) → SQLite; image → `media/` + thumb + metadata label `[image WxH]`
5. On **new** insert only: if `enable_auto_tag`, `apply_auto_tags` matches `auto_tag_rules` (content type OR keyword, case-insensitive) → `ensure_auto_tag` + `record_tags`. Hash-dedup updates skip retagging.
6. Emit `clipboard-changed`; Vue store updates list (refreshes tag counts when the record has tags)
7. Paste: write clipboard → (floating: hide panel) → restore previous foreground HWND → Ctrl+V. Target HWND remembered when panel opens. If no valid target, only updates clipboard.

### Frontend Component Tree
```
App.vue                          # Events (clipboard-changed, capture-paused, toggle-panel), ToastHost, ConfirmDialog
├── FloatingPanel.vue            # Floating UI; filters; empty trash; useBatchActions + useClipboardHotkeys
├── WindowApp.vue                # Window UI; SideBar; list-toolbar sort select; batch/hotkeys
│   ├── SearchBar.vue
│   ├── RecordList.vue           # Infinite scroll; thumbs; PreviewPane
│   │   └── PreviewPane.vue      # Header meta line; DOMPurify HTML preview; paste / tags / trash
│   └── SideBar.vue              # Categories (+ favorites); trash; tags (「自动」 badge); capture/theme/settings
├── SettingsWindow.vue           # Nav: 外观 → 快捷键 → 历史 → 标签（自动打标规则）→ 隐私 → 系统 → 数据 → 统计 → 帮助 → 关于
├── WindowControls.vue
├── ToastHost.vue
├── ConfirmDialog.vue / TagDialog.vue
├── composables/useBatchActions.ts · useClipboardHotkeys.ts · useToast.ts · useConfirm.ts
├── utils/mediaUrl.ts · sanitizeHtml.ts
└── TrayMenu.vue                 # Placeholder (native tray is Rust)
```

### Backend (Rust) Module Layout
- `lib.rs` — setup, command registration, `Settings` / `AutoTagRule`, capture path auto-tag hook, `show_main_panel` (paste-target HWND), shortcuts, ignore-list helpers, list IPC payload trim
- `commands.rs` — Tauri commands (CRUD, paste, settings, import/export, stats, mode switch)
- `window.rs` — adaptive / remembered size, round corners, resize persistence
- `tray.rs` — system tray menu / click
- `clipboard.rs` — monitor, paste-target HWND, write clipboard, focus restore + `keybd_event` Ctrl+V, suppress self-write
- `media.rs` — encode/store/load/delete
- `db.rs` — CRUD, FTS5, trash, tags (`ensure_auto_tag` / `apply_auto_tags`), `insert_record` → `(id, is_new)`, stats (`data_path`), settings, list/search `ORDER BY` whitelist
- `detect.rs` — content type + sensitive detection + SHA-256 helper
- `main.rs` — `clipvault_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, category×tag AND filters, trash exclusive, batch, pause, pagination (60 / `has_more`), `listSort` (session), `ensureRecordDetail` for HTML
- `settingsStore` — debounced auto-save (200ms); theme / appearance; `enable_auto_tag` + `auto_tag_rules`; applies CSS vars + `set_window_corner_radius`

### Key Design Decisions
- **Floating vs window:** Both borderless, `transparent: true`, `shadow: false`. Floating: always-on-top, hide on blur. Window: SideBar + `WindowControls` + list-toolbar. Shared `.panel-surface` chrome. **Size:** `resolve_panel_size` prefers last user resize (`floating_*` / `window_*` in settings); if unset (0), falls back to `adaptive_panel_size` (floating ≈ 40%×65%, window ≈ 55%×72%, clamped). Resize is debounced ~400ms into SQLite; maximized sizes are not saved. Frontend `save_settings` never overwrites size fields.
- **List sort (window mode):** Toolbar `<select>` → `clipboardStore.listSort` → `get_records` / `search_records` `sort` param. Whitelist: `updated_desc` (default), `updated_asc`, `created_desc`, `copies_desc`. Non-trash: `is_pinned DESC` first. Session-only (not in settings). `onNewRecord` prepends only for `updated_desc`; other sorts reload.
- **True round corners (Windows):** CSS `border-radius` alone leaves black rectangular corners on transparent WebView2. Clip HWND with `SetWindowRgn` from `panel_radius` × DPI. Command: `set_window_corner_radius`.
- **Theming:** CSS vars on `:root`; `.light-theme` / `.oled-theme` via `document.body.classList`. SideBar can toggle dark↔light.
- **Font size:** Root `font-size` = setting (default **16px**). Rem baseline is **16px** (`--ui-font-scale = font_size/16`). Main UI (list / preview / sidebar / floating chrome) uses `rem`; Settings page keeps fixed `px` so the settings UI does not jump while dragging the slider.
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s.
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Memory (frontend):** List soft-capped (`PAGE_SIZE * 2`) on `onNewRecord`. Full content/HTML live in a small `recordDetails` map (max ~6), never merged into list rows. `loadRecords`/search clears detail cache.
- **Clipboard fingerprint:** SHA-256 of text+html (not retaining full HTML string in `last_text_fp`).
- **Retention “回收站保留天数”:** Only purges trashed rows (not the whole history). **最大记录数** evicts oldest non-favorite / non-pinned when inserting.
- **Toast policy:** Actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML; preview loads via `get_record` / detail. Preview uses **DOMPurify + `v-html`** (not iframe) when markup differs from plain. Display CSS may force wrap; stored HTML for paste is unchanged. Manual select-copy from preview may normalize whitespace. Preview sanitization does **not** affect paste (original HTML is written back).
- **Preview chrome:** Type + actions in header; source / time / size-or-chars / 富文本 / 使用次数 as one meta line (`title` = content type). Single scroll on `.preview-content` (no nested scroll on content / rich HTML). Image preview: click → `open_record_media` opens the file with the OS default app (`cmd /c start`, path must stay under media root). Do not use `shell.open` for local files (default scope is http/https only).
- **Filters:** Type/favorites **AND** tag combine; trash is exclusive. IPC: `get_records` / `search_records` / `get_all_tags` use `#[tauri::command(rename_all = "snake_case")]` — pass `content_type`, `favorites_only`, `tag`, `trashed`, `sort`. Tag counts follow active category (`get_all_tags`). SideBar shows an 「自动」 badge for `is_auto` tags.
- **Auto-tag:** Settings `enable_auto_tag` (default **true**) + `auto_tag_rules`. Per-rule match is OR. Applied in a **single DB transaction** (batch `record_tags`). Defaults: 链接←`link`; 部署 / 前端←keyword lists. UI under Settings → 标签 (rules edited via local draft + 400ms commit debounce). Frontend `scheduleLoadTags` (350ms) coalesces `get_all_tags` after filter/tag bursts. List soft-cap also applies after `loadMore`.
- **Search:** FTS5 trigram (≥3 chars) on content / source_app / source_window / tags; shorter queries use escaped `LIKE`. FTS sync via triggers. **FTS v2:** use `DELETE FROM records_fts WHERE rowid=…` in triggers — the FTS5 `'delete'` command returns `SQL logic error` on current Windows SQLite and breaks empty-trash / permanent delete.
- **Stats storage:** `storage_bytes` ≈ text content length sum + cached `media/` dir size (not full SQLite file/index). `data_path` is the absolute app data dir shown on the stats page.
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set`.
- **Global shortcut:** From `settings.global_shortcut` at startup; re-bound in `save_settings`.
- **Pause capture:** Frontend + tray both update Rust; tray emits `capture-paused`.
- **Cleanup:** Expired/retention throttled (~60s).
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint (plain+html) or full image bytes. Hash match updates `copy_count` / `updated_at` (active rows only). **Paste self-write:** `paste_record` suppresses monitor emits ~1.5s so CF_HTML/plain round-trips don't insert a duplicate (fingerprint would otherwise diverge from the stored hash).
- **Paste focus:** On panel show (`show_main_panel` / tray / shortcut / single-instance), remember previous foreground HWND. Paste writes clipboard, hides floating panel, `SetForegroundWindow` target, then Ctrl+V. No valid target → clipboard only. `auto_close_on_paste` false → floating panel reopens after paste.
- **Hide-on-close / single instance / autostart:** tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles map 1:1 to tracker labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.
