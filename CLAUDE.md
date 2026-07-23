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
- **Database:** SQLite via rusqlite (WAL + `busy_timeout=5000`), `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media:** PNG + JPEG thumbs under `%LOCALAPPDATA%/ClipVault/media/`; DB stores paths/size only; column `content_len` stores text length at insert (backfilled once)
- **Clipboard polling:** arboard every 500ms, but **`GetClipboardSequenceNumber` skips all reads** when OS clipboard unchanged. **Text-first:** meaningful share text skips `get_image()`; otherwise only call it when `IsClipboardFormatAvailable` reports bitmap/DIB. Monitor **`try_send`s** to a bounded worker (`sync_channel(4)`); full queue drops the event (never blocks the poll thread). Image SHA-256 runs on the worker; poll only uses a cheap edge-sample fingerprint.
- **List IPC:** `substr(content,1,400)` + `content_len` column; `content_html` omitted. `clipboard-changed` emits the same light payload. Detail/`get_record` still full. **Export** uses `get_records_for_export` (full `content` + `content_html` + tags) — never reuse list columns.
- **List UI:** `RecordList` window-virtualizes rows (row height scales with `font_size`); soft-cap bounds in-memory pages; soft-cap dirty → next `loadMore` reloads. Default sort `loadMore` uses **keyset** (`before_pinned` / `before_updated_at` / `before_id`) to avoid OFFSET drift when new rows prepend. Floating panel stays mounted (`v-show`); `showPanel` reloads at most every ~30s unless empty.
- **Stats:** one SQL scan (aggregates + per-type CASE counts) + `SUM(content_len)`; `media/` size cached 120s and **incrementally adjusted** on image store/delete. Frontend `scheduleLoadStats`: 800ms debounce + 5s max-wait. Tag assign uses `set_record_tags` (one transaction + single FTS refresh).
- **Expire sweep:** watches expire fingerprint (`count:nearest`), not every list length change.
- **Appearance IPC:** `set_window_corner_radius` only when `panel_radius` changes.
- **Asset protocol:** `protocol-asset`; scope uses `$LOCALDATA/ClipVault/media/**/*` (not `$LOCALAPPDATA`)
- **Autostart / shortcut / ignore list:** applied from Rust on `save_settings` / setup (not frontend-only)

### Data Flow
1. `ClipboardMonitor` polls every 500ms; skip work when sequence / image quick-fp unchanged
2. **Image vs text:** Prefer text only for meaningful shares (≥16 chars, not URL-only). Screenshots / browser “Copy image” (URL-only text) → image
3. Skip capture when `source_app` matches `settings.ignored_apps`
4. Persist: text (+ optional `content_html`) → SQLite; image → `media/` + thumb + metadata label `[image WxH]`
5. On **new** insert only: if `enable_auto_tag`, `apply_auto_tags` matches `auto_tag_rules` (content type OR keyword, case-insensitive) → `ensure_auto_tag` + `record_tags` in one transaction, then **one** FTS refresh. Hash-dedup updates skip retagging.
6. Emit `clipboard-changed` (list-shaped payload); Vue store updates list (refreshes tag counts when the record has tags)
7. Paste: hide floating panel → `spawn_blocking` (`take_record_for_paste` + write clipboard + focus) → async 80ms sleep → `simulate_paste_keys`. Image paste prefers registered `"PNG"` clipboard format (file bytes); RGBA/`set_image` is fallback. Serialized via `tokio::sync::Mutex`. Target HWND remembered when panel opens. If no valid target, only updates clipboard.

### Frontend Component Tree
```
App.vue                          # Events; FloatingPanel v-show (warm); ToastHost, ConfirmDialog
├── FloatingPanel.vue            # Floating UI; filters; BatchBar; useBatchActions + useClipboardHotkeys
├── WindowApp.vue                # Window UI; SideBar; list-toolbar sort; BatchBar; hotkeys
│   ├── SearchBar.vue            # aria-label; / or Ctrl+K focus
│   ├── RecordList.vue           # Virtual listbox (role=listbox/option); ContextMenu; PreviewPane
│   │   └── PreviewPane.vue      # Paste primary CTA; icon-only delete; tags; trash
│   └── SideBar.vue              # Categories; trash; tags; ContextMenu; ≤720px icon rail
├── SettingsWindow.vue           # Nav: 外观 → … → 关于；theme radiogroup; ≤720px icon nav
├── BatchBar.vue                 # Shared batch actions (floating + window)
├── BaseDialog.vue               # Teleport + Esc + focus trap; shared dialog chrome
├── ConfirmDialog.vue / TagDialog.vue  # Content slots on BaseDialog
├── ContextMenu.vue              # Fixed + clamp; Arrow/Enter/Esc; role=menu
├── WindowControls.vue
├── ToastHost.vue
├── composables/useBatchActions.ts · useClipboardHotkeys.ts · useToast.ts · useConfirm.ts
├── utils/mediaUrl.ts · sanitizeHtml.ts
└── TrayMenu.vue                 # Placeholder (native tray is Rust)
```

### Backend (Rust) Module Layout
- `lib.rs` — setup, command registration, capture worker + **periodic cleanup thread** (~60s), `Settings` / `AutoTagRule`, `show_main_panel`, shortcuts, ignore-list helpers, `list_ipc_payload`
- `commands.rs` — Tauri commands (CRUD, paste on `spawn_blocking`, settings, import/export, stats, mode switch)
- `window.rs` — adaptive / remembered size, round corners, resize persistence. **Window mode** min width **760** (SideBar+List+Preview ≥740); floating stays compact.
- `tray.rs` — system tray menu / click
- `clipboard.rs` — monitor, paste-target HWND, write text/PNG/image, focus restore + Ctrl+V keys, suppress self-write (**do not advance `last_*` fingerprints while suppressed**)
- `media.rs` — encode/store/load/delete; media dir size cache
- `db/` — SQLite layer (`mod.rs` core CRUD/schema/FTS; `tags.rs` tag CRUD + auto-tag; `stats.rs` aggregates). **WAL:** write `conn` + **read pool** (3× `query_only`). `content_len` column. Export: `get_records_for_export`.
- `detect.rs` — content type + sensitive detection + SHA-256 helpers
- `main.rs` — `clipvault_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, category×tag AND filters, trash exclusive, batch, pause, pagination (60 / `has_more`), keyset/`listFetchOffset`, `listSort` (session), `ensureRecordDetail` for HTML; `loadRecords`/search re-fetches detail for current selection
- `settingsStore` — debounced auto-save (200ms); theme / appearance; `enable_auto_tag` + `auto_tag_rules`; applies CSS vars + body classes (`blur-enabled`, `mode-window` / `mode-floating`) + `set_window_corner_radius`

### Key Design Decisions
- **Brand:** Product name **ClipVault** everywhere (title bar, floating panel, about, `tauri.conf` window title). Version lives on the About page only.
- **Floating vs window:** Both borderless, `transparent: true`, `shadow: false`. Floating: always-on-top, hide on blur; panel kept mounted with `v-show`. Window: SideBar + `WindowControls` + list-toolbar; `mode_size_bounds` min width **760**. Shared `.panel-surface` chrome. **Size:** `resolve_panel_size` prefers last user resize (`floating_*` / `window_*` in settings); if unset (0), falls back to `adaptive_panel_size`. Resize is debounced ~400ms into SQLite; maximized sizes are not saved. Frontend `save_settings` never overwrites size fields (`SIZE_SAVE_GEN`).
- **List sort (window mode):** Toolbar `<select>` → `clipboardStore.listSort` → `get_records` / `search_records` `sort` param. Whitelist: `updated_desc` (default), `updated_asc`, `created_desc`, `copies_desc`. Non-trash: `is_pinned DESC` first. Session-only. `onNewRecord` prepends only for `updated_desc`; other sorts reload (debounced ~400ms).
- **True round corners (Windows):** CSS `border-radius` alone leaves black rectangular corners on transparent WebView2. Clip HWND with `SetWindowRgn` from `panel_radius` × DPI. Command: `set_window_corner_radius`.
- **Theming / tokens:** CSS vars on `:root` (incl. `--type-*`, `--text-xs`…`--text-xl`, `--space-*`, `--win-close-hover`). Themes: `.light-theme` / `.oled-theme`. Type badges use global `.badge-*` + `--type-*` only (no per-component hardcodes). SideBar can toggle dark↔light.
- **Blur:** Setting `enable_blur` applies `backdrop-filter` **only in floating mode**. Window mode always skips blur (`body.mode-window`) to avoid full-viewport compositing cost; setting remains for when the user returns to floating.
- **Font size:** Root `font-size` = setting (default **16px**). Rem baseline is **16px** (`--ui-font-scale = font_size/16`). Prefer `rem` / `--text-*` so Settings / dialogs scale with the user preference. Virtual list row height scales with `font_size`.
- **Responsive (window):** `@media (max-width: 720px)` — SideBar / settings nav → icon rail; preview actions denser grid; theme cards 2×2.
- **A11y (baseline):** Record list `role="listbox"` / `option` + roving tabindex; dialogs via `BaseDialog` (Esc + focus trap); `ContextMenu` keyboard + clamp; global `:focus-visible`; theme cards `role="radio"`; form `aria-label`s on search / ranges / ignore-app input. Tertiary text colors raised for WCAG-ish contrast.
- **Preview actions:** 「粘贴」is `action-primary` (solid accent); delete is icon-only. Pin available via header / hotkey / context menu when the narrow grid hides it.
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s. `is_sensitive` is a **bool**, not a `content_type` (ContentType = text|code|link|image|file only).
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Memory (frontend):** List soft-capped (`PAGE_SIZE * 2`) on `onNewRecord` / `loadMore`. Full content/HTML in `recordDetails` (max ~6). Batch copy fetches full text via `get_record` (list rows are truncated).
- **Clipboard fingerprint:** SHA-256 of text+html (not retaining full HTML string in `last_text_fp`). Image poll: quick-fp only; worker computes full hash.
- **Retention “回收站保留天数”:** Only purges trashed rows. **最大记录数** evicts oldest non-favorite / non-pinned when inserting (write lock).
- **Toast policy:** Actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles. Failed tag create/assign must toast error.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML; preview loads via `get_record` / detail. Preview uses **DOMPurify + `v-html`**. Paste writes original HTML back.
- **Preview chrome:** Type + actions in header; source / time / size-or-chars / 富文本 / 使用次数 as one meta line. Image preview: click → `open_record_media` (`cmd /c start` under media root). Do not use `shell.open` for local files.
- **Filters:** Type/favorites **AND** tag combine; trash is exclusive. IPC: `get_records` / `search_records` / `get_all_tags` use `rename_all = "snake_case"`. Tag counts follow active category. SideBar 「自动」 badge for `is_auto` tags.
- **Auto-tag:** Settings `enable_auto_tag` (default **true**) + `auto_tag_rules`. Per-rule match is OR. No per-tag FTS triggers — refresh FTS once after batch tag writes (**FTS v3**). Defaults: 链接←`link`; 部署 / 前端←keywords. UI: Settings → 标签 (local draft + 400ms commit). `scheduleLoadTags` 350ms.
- **Search:** FTS5 trigram (**≥3 chars**) on content / source_app / source_window / tags. **1–2 chars:** single-pass `instr(...)` + tag `EXISTS` (no `LIKE '%X%'`). FTS update trigger is **`OF content` only** so hash-dedup source updates do not rebuild FTS. Tag changes call `refresh_record_fts`. **FTS delete:** `DELETE FROM records_fts WHERE rowid=…` (not FTS5 `'delete'` command — broken on Windows SQLite).
- **Stats storage:** `storage_bytes` ≈ `SUM(content_len)` (+ HTML lengths) + cached `media/` dir size. `data_path` is the absolute app data dir on the stats page.
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set`.
- **Global shortcut:** From `settings.global_shortcut` at startup; re-bound in `save_settings`.
- **Pause capture:** Frontend + tray both update Rust; tray emits `capture-paused`.
- **Cleanup:** Independent background thread (~60s): `cleanup_expired` + `cleanup_retention`. Not on the capture hot path. Frontend expire sweep + `records-expired` event sync the list.
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint (plain+html) or full image bytes. Check + update/insert under the **same write Mutex**. Hash match updates `copy_count` / `updated_at` / source (active rows only).
- **Paste self-write:** `paste_record` suppresses monitor emits ~1.5s. While suppressed, **do not advance** `last_text_fp` / `last_image_hash` — otherwise a real copy in that window is permanently lost. Re-capture of our own paste after the window is OK (DB hash dedupes).
- **Paste focus:** On panel show, remember previous foreground HWND. Paste writes clipboard (PNG bytes preferred for images), hides floating panel, focuses target, async delay, Ctrl+V. No valid target → clipboard only. `auto_close_on_paste` false → floating panel reopens.
- **Hide-on-close / single instance / autostart:** tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.
- **UI review:** Historical findings + batch status in [`docs/ui-design-review.md`](docs/ui-design-review.md).

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles map 1:1 to tracker labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.
