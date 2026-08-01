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
npm run doctor       # 环境诊断：Node / Rust / WebView2 / SQLite，异常时输出修复建议
npm run clippy       # Rust clippy（-D warnings，与 CI 一致）

cargo test --manifest-path src-tauri/Cargo.toml   # Run Rust backend tests
```

The full Tauri dev command is `npm run tauri dev` (starts both Vite + Rust backend).

**After modifying Rust code** (`src-tauri/src/*.rs`), run `cargo test --manifest-path src-tauri/Cargo.toml` to verify the backend still passes its tests.

CI (`.github/workflows/ci.yml`) runs on every push / PR: frontend lint, type-check + build and vitest on Ubuntu; Rust `cargo clippy -- -D warnings` and `cargo test` on Windows (`windows-latest`, because the Rust code is Windows-gated). Run `npm run lint` / `npm test` / `npm run clippy` locally before pushing.

Regenerate app icons from a source image (PNG preferred; JPEG renamed as `.png` must be converted first):

```bash
npx tauri icon app-icon.png -o src-tauri/icons
```

## Architecture

Clipboard is a **Tauri v2** desktop clipboard manager for Windows.

### Stack
- **Frontend:** Vue 3 + TypeScript + Vite + Pinia; Lucide icons; DOMPurify for rich-text preview
- **Backend:** Rust (Tauri v2 plugins: single-instance, clipboard-manager, dialog, global-shortcut, autostart). No fs/shell/sql plugins — filesystem & SQLite stay in Rust commands/`rusqlite`; media open uses `ShellExecuteW`
- **Database:** SQLite via rusqlite (WAL + `busy_timeout=5000`), `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media:** PNG + JPEG thumbs under `%LOCALAPPDATA%/ClipVault/media/`; DB stores paths/size only; column `content_len` stores text length at insert (backfilled once). Capture/store max edge **2560**; list thumb max edge **160**.
- **Clipboard polling:** arboard every 500ms, but **`GetClipboardSequenceNumber` skips all reads** when OS clipboard unchanged. **Text-first:** meaningful share text skips `get_image()`; otherwise only call it when `IsClipboardFormatAvailable` reports bitmap/DIB. Monitor **`try_send`s** to a bounded worker (`sync_channel(2)`); full queue drops the event (never blocks the poll thread). Large images are downscaled on the poll thread before enqueue. Image SHA-256 runs on the worker; poll only uses a cheap edge-sample fingerprint.
- **List IPC:** `substr(content,1,400)` + `content_len` column; `content_html` omitted. `clipboard-changed` emits the same light payload. Detail/`get_record` still full. **Export** uses `get_records_for_export` (full `content` + `content_html` + tags) — never reuse list columns.
- **List UI:** `RecordList` window-virtualizes rows via the `useVirtualList` composable (row height scales with `font_size`; grid rows grouped in JS). Grid column count is a **single JS source of truth** (`gridCols` from ResizeObserver, inline `grid-template-columns`) — never CSS `auto-fill`, which would drift from the virtualizer's row grouping (ADR-0001). Toolbar (`ListToolbar`) + empty/loading state (`ListEmptyState`) are child components; toolbar renders only in window mode (`showListChrome`). Soft-cap bounds in-memory pages; soft-cap dirty → next `loadMore` reloads. Default sort `loadMore` uses **keyset** (`before_pinned` / `before_updated_at` / `before_id`) to avoid OFFSET drift when new rows prepend. Floating panel stays mounted (`v-show`); `showPanel` reloads at most every ~30s unless empty.
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
7. Paste: write clipboard → focus previous app → floating **hide** / window **minimize** when auto-close → Ctrl+V. Image paste prefers registered `"PNG"` clipboard format (file bytes); RGBA/`set_image` is fallback. Serialized via `tokio::sync::Mutex`. Target HWND remembered when panel opens / foreign FG tracked. If no valid target, only updates clipboard.

### Frontend Component Tree
```
App.vue                          # Events; FloatingPanel v-show; WelcomeDialog; ToastHost, ConfirmDialog
├── FloatingPanel.vue            # Floating UI; filters; CaptureStatus; BatchBar; useBatchActions + useClipboardHotkeys
├── WindowApp.vue                # Window UI; SideBar; hotkeys; sidebar resizer (useColumnResize)
│   ├── SearchBar.vue            # aria-label; / or Ctrl+K focus
│   ├── RecordList.vue           # Virtual listbox (useVirtualList); cards/grid; ContextMenu; BatchBar; AliasDialog; list/preview resizer
│   │   ├── ListToolbar.vue      # Window-only chrome: category title, sort select, list/grid toggle, empty-trash
│   │   ├── ListEmptyState.vue   # Loading / empty state
│   │   └── PreviewPane.vue      # Paste primary CTA; icon-only delete; tags; trash
│   └── SideBar.vue              # Categories; trash; tags; ContextMenu; ≤720px icon rail
├── SettingsWindow.vue           # Nav + section router; shortcut-recording window listener; ≤720px icon nav
│   └── settings/Settings*.vue   # 11 sections (shortcuts/appearance/history/tags/privacy/stats/data/sync/system/help/about)
│                                #   shared store access via composables/useSettings.ts; primitives in styles/settings.css
├── WelcomeDialog.vue            # First-run welcome (BaseDialog); onboarding_completed
├── BatchBar.vue                 # Shared batch actions (floating + window)
├── ToggleSwitch.vue             # Shared switch primitive (settings sections)
├── SourceBadge.vue              # Source-app letter avatar + short name
├── BaseDialog.vue               # Teleport + Esc + focus trap; shared dialog chrome
├── ConfirmDialog.vue / TagDialog.vue / AliasDialog.vue  # Content slots on BaseDialog
├── ContextMenu.vue              # Fixed + clamp; Arrow/Enter/Esc; role=menu
├── WindowControls.vue
├── ToastHost.vue
├── TrayMenuApp.vue              # Custom tray-menu window entry (Vite multi-page)
├── composables/useVirtualList.ts · useColumnResize.ts · useSettings.ts · useBatchActions.ts · useClipboardHotkeys.ts · useToast.ts · useConfirm.ts · useBatchBarHeight.ts · pasteFocusLock.ts
└── utils/mediaUrl.ts · sanitizeHtml.ts · trayMenuItems.ts
```

### Backend (Rust) Module Layout
- `lib.rs` — setup, command registration, capture worker + **periodic cleanup thread** (~60s), `Settings` / `AutoTagRule` / `onboarding_completed`, `show_main_panel`, shortcuts, ignore-list helpers, `list_ipc_payload`
- `commands.rs` — Tauri commands (CRUD, paste, settings, import/export, stats, mode switch, `tray_menu_action` / `get_tray_menu_state`)
- `window.rs` — adaptive / remembered size, round corners, resize persistence. **Window mode** min width **760** (SideBar+List+Preview ≥740); floating stays compact.
- `tray.rs` — tray icon (no native menu); right-click shows `tray-menu` window; left-click → `toggle_main_panel` (see Custom tray menu); **Windows power-resume** rebuilds tray + reloads webviews
- `system_theme.rs` — Windows OS light/dark watcher: invisible top-level window receives the `WM_SETTINGCHANGE`("ImmersiveColorSet") broadcast, reads the `AppsUseLightTheme` registry value, emits `system-theme-changed` (ADR-0002)
- `clipboard.rs` — monitor, paste-target HWND, write text/PNG/image, focus restore + Ctrl+V keys, suppress self-write (**do not advance `last_*` fingerprints while suppressed**); capture downscale ≤2560 edge
- `media.rs` — encode/store/load/delete (max edge **2560**, thumb **160**); media dir size cache
- `db/` — SQLite layer (`mod.rs` core CRUD/schema/FTS; `tags.rs` tag CRUD + auto-tag; `stats.rs` aggregates). **WAL:** write `conn` + **read pool** (3× `query_only`). `content_len` column. Export: `get_records_for_export`.
- `detect.rs` — content type + sensitive detection + SHA-256 helpers
- `webdav/` — WebDAV cloud sync (`client.rs` HTTP client; `sync.rs` pull/merge/push orchestration). Protocol `clipvault-webdav-v1`; manifest + JSONL bundle; media files synced alongside. Settings page: **Sync** (`SettingsSync.vue`). Default remote dir `ClipVaultSync`.
- `security.rs` — media path must resolve under media root; export/import JSON path checks; import normalizes `content_type` and allows only http(s) links + safe media rel-paths
- `main.rs` — `clipboard_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, category×tag AND filters, trash exclusive, batch, pause, pagination (60 / `has_more`), keyset/`listFetchOffset`, `listSort` (session), `ensureRecordDetail` for HTML; `loadRecords`/search re-fetches detail for current selection
- `settingsStore` — debounced auto-save (200ms); theme / appearance (**"system" theme follows the OS**: native `system-theme-changed` event primary, matchMedia fallback; `lastKnownSystemDark` cache outranks stale matchMedia — ADR-0002); `enable_auto_tag` + `auto_tag_rules`; `onboarding_completed`; applies CSS vars + body classes (`blur-enabled`, `mode-window` / `mode-floating`) + `set_window_corner_radius`

### Key Design Decisions
- **Brand:** Product name **Clipboard** everywhere (title bar, floating panel, about, `tauri.conf` window title). Version lives on the About page only.
- **First-run onboarding:** `WelcomeDialog` when `onboarding_completed` is false. New install Default=`false`; **upgrade** JSON missing the field deserializes to `true` (skip). Dismiss / Esc sets true and saves. Spec: `docs/superpowers/specs/2026-07-24-onboarding-design.md`.
- **Floating vs window:** Both borderless, `transparent: true`, `shadow: false`. Floating: always-on-top, hide on blur; panel kept mounted with `v-show`. Window: SideBar + `WindowControls` + list-toolbar; `mode_size_bounds` min width **760**. Shared `.panel-surface` chrome. **Size:** `resolve_panel_size` prefers last user resize (`floating_*` / `window_*` in settings); if unset (0), falls back to `adaptive_panel_size`. Resize is debounced ~400ms into SQLite; maximized sizes are not saved. Frontend `save_settings` never overwrites size fields (`SIZE_SAVE_GEN`).
- **List sort (window mode):** Toolbar `<select>` → `clipboardStore.listSort` → `get_records` / `search_records` `sort` param. Whitelist: `updated_desc` (default), `updated_asc`, `created_desc`, `copies_desc`. Non-trash: `is_pinned DESC` first. Session-only. `onNewRecord` prepends only for `updated_desc`; other sorts reload (debounced ~400ms).
- **True round corners (Windows):** CSS `border-radius` alone leaves black rectangular corners on transparent WebView2. Clip HWND with `SetWindowRgn` from `panel_radius` × DPI. Command: `set_window_corner_radius`.
- **Source badge:** List + preview show a 14px letter avatar + short name via `SourceBadge` / `sourceBadge.ts`. Empty source →「系统剪贴板」/「剪」/ gray. Real exe icons later via optional `iconSrc`.
- **Follow-system theme (Windows):** WebView2 does not reliably fire matchMedia change events while its window is hidden — and the panel is hidden most of the time. A Rust watcher (invisible top-level window + `WM_SETTINGCHANGE`/`ImmersiveColorSet` + `AppsUseLightTheme` registry) emits `system-theme-changed`; frontends apply it only when `theme === "system"`, and the `lastKnownSystemDark` cache outranks stale matchMedia on any re-application (ADR-0002). Windows「夜间模式」(night light) is undetectable and not covered.
- **Theming / tokens:** CSS vars on `:root` (incl. `--type-*`, `--pin` / `--pin-soft`, `--text-xs`…`--text-xl`, `--space-*`, `--win-close-hover`). Themes: `.light-theme` / `.oled-theme`. SideBar can toggle dark↔light.
  - **Accent:** Fluent blue `--accent: #0078d4` (dark + light). Hover/light variants: dark `#1b86d9` / `#60cdff`; light hover `#106ebe`. Focus rings / primary CTA /「全部」nav use accent.
  - **Column surfaces:** SideBar `--bg-elevated`; list + preview share `--bg-surface` (content band). Separated by a single list `border-right`.
  - **Type colors (`--type-*`):** text sky `#7dd3fc` · code green `#34d399` · link deep blue (dark `#60a5fa` for AA contrast / light `#2563eb`) · image cyan `#0ea5e9` · file amber `#eab308`. Badges (`.badge-*`), SideBar category active (`--cat-color`), and type icons / link titles follow these. List **selection/hover** is Fluent flat (accent soft fill / `--bg-hover`), not type-colored card borders.
  - **Pin vs favorite:** `--pin` violet (dark `#a78bfa` / light `#7c3aed`) for pinned UI — not red, so it stays distinct from `--danger`; `--warning` gold for favorites. Preview bottom bar uses `action-pinned` vs `action-active` — do not share one “active” style for both.
  - **Pinned list chrome:** 「置顶」section label (pin color) + hairline divider before the first unpinned row when both groups exist (virtual-list `divider` item).
  - **Tag palette:** `themeColors.ts` resolves 12 presets from `--accent` / `--type-*` / status tokens at runtime; no free-form color picker. Existing SQLite hex values are left as-is.
- **Blur:** Setting `enable_blur` defaults **false**. Frosted glass comes from the **native DWM acrylic** backdrop (`set_window_backdrop` → `Effect::Acrylic`; Win11 `DWMSBT_TRANSIENTWINDOW`, Win10 `ACCENT_ENABLE_ACRYLICBLURBEHIND`) — CSS `backdrop-filter` cannot blur the OS desktop behind a transparent WebView2. When on, `body.blur-enabled` also makes `.panel-surface` (and tray-menu) backgrounds translucent so the blurred desktop shows through. Intensity adjustable via `blur_strength` (30–80%, default 45): surface tint opacity = `100 − blur_strength` (CSS var `--panel-blur-opacity`). Applies in both floating and window mode.
- **Custom tray menu:** Separate `tray-menu` WebView (Vite multi-page). Right-click anchors above tray icon; theme/blur follow settings (incl. live `system-theme-changed` events while following the OS). **Left-click** → `toggle_main_panel`: hidden/minimized → show + focus; visible but not foreground → bring to front (`show_main_panel` / `focus_window`); already foreground → hide. After sleep/wake, power watcher rebuilds tray + reloads webviews.
- **Font size:** Root `font-size` = setting (default **16px**). Rem baseline is **16px** (`--ui-font-scale = font_size/16`). Prefer `rem` / `--text-*` so Settings / dialogs scale with the user preference. Virtual list row height scales with `font_size`.
- **Responsive (window):** `@media (max-width: 720px)` — SideBar / settings nav → icon rail (sidebar resizer hidden); preview actions denser grid; theme cards 2×2.
- **Column resize:** SideBar width and list-column width are user-draggable (`useColumnResize` composable: pointer events + rAF throttle). Widths persist in localStorage (`clipboard-sidebar-width`, `clipboard-list-col-width`). List column always uses its stored width (no jump on preview open/close); first run captures the natural flex width via DOM measurement. Sidebar resize disabled ≤720px (icon-rail mode).
- **Motion / animation:** Follow `docs/Clipboard-交互动效规范.md`. **Never `transition: all`** — always an explicit property list. Prefer compositor-friendly props (`opacity` / `transform`); don't continuously animate layout (`padding`/`margin`/`height`/`grid-template-rows`) or `background` paint. All durations come from `--transition-*` tokens — no hardcoded `0.15s` etc. BatchBar floats **absolute** over the list (`batch-bar-holder`, main.css) so toggling batch mode never reflows the list; hosts reserve its height via `useBatchBarHeight` (ResizeObserver) as transitioned top `padding`. Column resizers (`div.resizer`) also overlay (`margin-left: -4px` + `z-index: 10`) instead of reserving flex space. New-record flash animates `opacity` on a `::before` overlay, not `background`. Loading/empty ↔ list uses `<Transition name="fade" mode="out-in">`.
- **A11y (baseline):** Record list `role="listbox"` / `option` + roving tabindex; dialogs via `BaseDialog` (Esc + focus trap); `ContextMenu` keyboard + clamp; global `:focus-visible`; theme cards `role="radio"`; form `aria-label`s on search / ranges / ignore-app input. Tertiary text colors raised for WCAG-ish contrast.
- **Preview actions:** 「粘贴」is `action-primary` (solid accent); delete is icon-only. Pin and favorite are on the bottom action bar / hotkey / context menu / list row (not in preview header).
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s. `is_sensitive` is a **bool**, not a `content_type` (ContentType = text|code|link|image|file only).
- **Color swatch (not a type):** If plain `text` content is a standalone CSS color (`#rgb` / `#rrggbb` / `rgb()` / `hsl()`, whole string), list shows a swatch chip and preview shows a large swatch — still `content_type: text`.
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Memory (frontend):** List soft-capped (`PAGE_SIZE * 2`) on `onNewRecord` / `loadMore`. Full content/HTML in `recordDetails` (max ~6). Batch copy fetches full text via `get_record` (list rows are truncated).
- **Clipboard fingerprint:** SHA-256 of text+html (not retaining full HTML string in `last_text_fp`). Image poll: quick-fp only; worker computes full hash.
- **Retention “回收站保留天数”:** Only purges trashed rows. **最大记录数** evicts oldest non-favorite / non-pinned when inserting (write lock).
- **Toast policy:** Actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles. Failed tag create/assign must toast error. Host: top-right (`top: 60px`) so toasts clear the title-bar controls.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML; preview loads via `get_record` / detail. Preview uses **DOMPurify + `v-html`**. Paste writes original HTML back.
- **Preview chrome:** Type + meta in header (no pin/favorite buttons); source / time / size-or-chars / 富文本 / 粘贴次数 as one meta line. **Borders:** header bottom divider only (no `preview-actions` / `sidebar-bottom` top rules). Text body has no box border; link/file use elevated fill without stroke; image thumb uses a hairline outline for contrast. **Spacing:** `.preview-tags` `8px 20px 16px`; `.preview-actions` `8px 20px 20px` (horizontal 20px aligns with content). Image preview: click → `open_record_media` (canonicalize under media root; Windows `ShellExecuteW` — not `cmd /c start` / `shell.open`). Preview links only allow `http:` / `https:`.
- **Filters:** Type/favorites **AND** tag combine; trash is exclusive. IPC: `get_records` / `search_records` / `get_all_tags` use `rename_all = "snake_case"`. Tag counts follow active category. SideBar: zero-count tags fold under「更多」; `is_auto` tags show a sparkles icon + tooltip「自动打标规则创建」(active zero-count tag stays in the primary list).
- **Record alias:** Optional short `alias` (max 80 chars) for display only — does **not** change paste content / hash / HTML. List title prefers alias (hover `title` = content preview). Edit via preview header or context menu (`set_record_alias`). Hash-dedup re-copy keeps existing alias. Import/export include `alias` (serde default `""`).
- **Auto-tag:** Settings `enable_auto_tag` (default **true**) + `auto_tag_rules`. Per-rule match is OR. No per-tag FTS triggers — refresh FTS once after batch tag writes (**FTS v4**). Defaults: 链接←`link`; 部署 / 前端←keywords. UI: Settings → 标签 (local draft + 400ms commit). `scheduleLoadTags` 350ms.
- **Search:** FTS5 trigram (**≥3 chars**) on content / alias / source_app / source_window / tags. **1–2 chars:** single-pass `instr(...)` (incl. `alias`) + tag `EXISTS` (no `LIKE '%X%'`). FTS update trigger is **`OF content` only** so hash-dedup source updates do not rebuild FTS. Tag / alias changes call `refresh_record_fts`. **FTS delete:** `DELETE FROM records_fts WHERE rowid=…` (not FTS5 `'delete'` command — broken on Windows SQLite).
- **Stats storage:** `storage_bytes` ≈ `SUM(content_len)` (+ HTML lengths) + cached `media/` dir size. `data_path` is the absolute app data dir; displayed on the **Data** settings page (moved from Stats).
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set`.
- **Global shortcut:** From `settings.global_shortcut` at startup; re-bound in `save_settings`.
- **Pause capture:** Frontend + tray both update Rust; tray emits `capture-paused`.
- **Cleanup:** Independent background thread (~60s): `cleanup_expired` + `cleanup_retention`. Not on the capture hot path. Frontend expire sweep + `records-expired` event sync the list.
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint (plain+html) or full image bytes. Check + update/insert under the **same write Mutex**. Hash match updates `updated_at` / source (active rows only) — does **not** bump `copy_count`. `copy_count` starts at **0** and increments only on paste from Clipboard.
- **Source app:** Foreground process via `QueryFullProcessImageNameW` + `PROCESS_QUERY_LIMITED_INFORMATION` (not `GetModuleFileNameW`, which only works for the current process). Empty `source_app` falls back to UI label「系统剪贴板」.
- **Paste self-write:** `paste_record` suppresses monitor emits ~1.5s. While suppressed, the poll loop skips all reads **and does not commit the sequence watermark** (nor `last_text_fp` / `last_image_hash`) — otherwise a real copy in that window is permanently lost. The first poll after the window re-reads the current clipboard; our own paste is then absorbed by DB hash-dedup.
- **Paste focus:** On panel show, remember previous foreground HWND. Paste writes clipboard (PNG bytes preferred for images), focuses target while still holding FG, then floating hide / window minimize when auto-close, Ctrl+V. No valid target → clipboard only. `auto_close_on_paste` false → restore panel (unminimize/show) without stealing focus.
- **Hide-on-close / single instance / autostart:** tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.
- **Architecture decisions:** [`docs/adr/`](docs/adr/) — ADR-0001 covers the virtual-list composable extraction and the responsive grid-column single-source-of-truth rule; ADR-0002 covers the native OS-theme watcher behind "follow system".
- **Tray / onboarding specs:** [`docs/superpowers/specs/`](docs/superpowers/specs/).

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles map 1:1 to tracker labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.
