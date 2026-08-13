# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
npm run dev          # Start Vite dev server (port 1420)
npm run build        # vue-tsc type-check + vite build
npm run tauri        # Run Tauri CLI commands (e.g., npm run tauri dev)
npm test             # Run Vitest once (components / stores / utils, jsdom)
npm run lint         # Run ESLint over src (.ts + .vue)
npm run doctor       # 环境诊断：Node / Rust / WebView2 / SQLite，异常时输出修复建议
npm run clippy       # Rust clippy（-D warnings，与 CI 一致）
npm run typecheck    # vue-tsc --noEmit（build 的前半段）
npm run check:ipc-contract  # Rust 命令签名 ↔ TS invoke 契约校验
npm run check:schema        # SQLite 建表 / ALTER 迁移一致性校验
npm run validate     # 本地全量校验（lint + typecheck + check:* + test + clippy + cargo-test）

cargo test --manifest-path src-tauri/Cargo.toml   # Run Rust backend tests
```

The full Tauri dev command is `npm run tauri dev` (starts both Vite + Rust backend).

**After modifying Rust code** (`src-tauri/src/*.rs`), run `cargo test --manifest-path src-tauri/Cargo.toml` to verify the backend still passes its tests.

CI (`.github/workflows/ci.yml`) runs on every push / PR: frontend lint, type-check + build, vitest, `check:schema` and `check:ipc-contract` on Ubuntu; Rust `cargo clippy -- -D warnings`, `cargo fmt --check` and `cargo test` on Windows (`windows-latest`, because the Rust code is Windows-gated). Run `npm run lint` / `npm test` / `npm run clippy` / `cargo fmt` locally before pushing.

Regenerate app icons from a source image (PNG preferred; JPEG renamed as `.png` must be converted first):

```bash
npx tauri icon app-icon.png -o src-tauri/icons
```

## Architecture

Clipboard is a **Tauri v2** desktop clipboard manager for Windows.

### Stack
- **Frontend:** Vue 3 + TypeScript + Vite + Pinia; Lucide icons; DOMPurify for rich-text preview
- **Backend:** Rust (Tauri v2 plugins: single-instance, dialog, global-shortcut, autostart). No fs/shell/sql/clipboard-manager plugins — filesystem & SQLite stay in Rust commands/`rusqlite`; clipboard I/O uses arboard; media open uses `ShellExecuteW`
- **Database:** SQLite via rusqlite (WAL + `busy_timeout=5000`), `%LOCALAPPDATA%/ClipVault/clipvault.db`
- **Media:** PNG + JPEG thumbs under `%LOCALAPPDATA%/ClipVault/media/`; DB stores paths/size only; column `content_len` stores text length at insert (backfilled once). Capture/store max edge **2560**; list thumb max edge **160**.
- **Clipboard monitor (Windows):** event-driven — message-only window + `AddClipboardFormatListener` (`WM_CLIPBOARDUPDATE`) with a **150ms debounce timer** folding the multiple notifications one logical copy emits into a single read. A **1s sequence-number watchdog** covers sleep/wake catch-up, retry after `ClipboardOccupied` (250ms fast retry), and graceful fallback if listener registration fails. Non-Windows keeps a 250ms poll loop. **Text-first:** meaningful share text skips `get_image()`; otherwise only call it when `IsClipboardFormatAvailable` reports bitmap/DIB. Monitor **`try_send`s** to bounded workers (`sync_channel(4)` text / `(2)` image); full queue drops the event (never blocks the monitor thread). Large images are downscaled on the monitor thread before enqueue. Image SHA-256 runs on the worker; monitor only uses a cheap edge-sample fingerprint. See ADR-0005.
- **List IPC:** `substr(content,1,400)` + `content_len` column; `content_html` omitted. `clipboard-changed` emits the same light payload. Detail/`get_record` still full. **Export** uses `get_records_for_export` (full `content` + `content_html` + tags) — never reuse list columns.
- **List UI:** `RecordList` window-virtualizes rows via the `useVirtualList` composable (row height scales with `font_size`; grid rows grouped in JS). Grid column count is a **single JS source of truth** (`gridCols` from ResizeObserver, inline `grid-template-columns`) — never CSS `auto-fill`, which would drift from the virtualizer's row grouping (ADR-0001). Toolbar (`ListToolbar`) + empty/loading state (`ListEmptyState`) are child components. Soft-cap bounds in-memory pages; soft-cap dirty → next `loadMore` reloads. Default sort `loadMore` uses **keyset** (`before_pinned` / `before_updated_at` / `before_id`) to avoid OFFSET drift when new rows prepend. `showPanel` reloads at most every ~30s unless empty.
- **Stats:** one SQL scan (aggregates + per-type CASE counts) + `SUM(content_len)` + `SUM(length(content_html))`; backend **5s TTL cache**; `media/` size cached 120s and **incrementally adjusted** on image store/delete. Frontend `scheduleLoadStats`: 800ms debounce + 5s max-wait. Tag assign uses `set_record_tags` (one transaction + single FTS refresh).
- **Expire sweep:** watches expire fingerprint (`count:nearest`), not every list length change.
- **Appearance IPC:** `set_window_corner_radius` only when `panel_radius` changes.
- **Asset protocol:** `protocol-asset`; scope uses `$LOCALDATA/ClipVault/media/**/*` (not `$LOCALAPPDATA`)
- **Autostart / shortcut / ignore list:** applied from Rust on `save_settings` / setup (not frontend-only)

### Data Flow
1. `ClipboardMonitor` (Windows) wakes on `WM_CLIPBOARDUPDATE` (debounced 150ms) or the 1s watchdog; skip work when sequence / image quick-fp unchanged
2. **Image vs text:** Prefer text only for meaningful shares (≥16 chars, not URL-only). Screenshots / browser “Copy image” (URL-only text) → image. “URL-only” includes http(s)/ftp/**magnet/ed2k/thunder** (`is_primarily_url` prefixes stay aligned with `security::LINK_PREFIXES`)
3. Skip capture when `source_app` matches `settings.ignored_apps`
4. Persist: text (+ optional `content_html`) → SQLite; image → `media/` + thumb + metadata label `[image WxH]`
5. On **new** insert only: if `enable_auto_tag`, `apply_auto_tags` matches `auto_tag_rules` (content type OR keyword, case-insensitive) → `ensure_auto_tag` + `record_tags` in one transaction, then **one** FTS refresh. Hash-dedup updates skip retagging.
6. Emit `clipboard-changed` (list-shaped payload); Vue store updates list (refreshes tag counts when the record has tags)
7. Paste: write clipboard → focus previous app → minimize window when auto-close → Ctrl+V. Image paste prefers registered `"PNG"` clipboard format (file bytes); RGBA/`set_image` is fallback. Serialized via `tokio::sync::Mutex`. Target HWND remembered when panel opens / foreign FG tracked. If no valid target, only updates clipboard.

### Frontend Component Tree
```
App.vue                          # Events; WindowApp; WelcomeDialog; ToastHost, ConfirmDialog
├── WindowApp.vue                # Window UI; SideBar; hotkeys; sidebar resizer (useColumnResize)
│   ├── SearchBar.vue            # aria-label; / or Ctrl+K focus
│   ├── RecordList.vue           # Virtual listbox (useVirtualList); cards/grid; ContextMenu; BatchBar; AliasDialog; list/preview resizer
│   │   ├── ListToolbar.vue      # Category title, sort select, list/grid toggle, empty-trash
│   │   ├── ListEmptyState.vue   # Loading / empty state
│   │   └── PreviewPane.vue      # Paste primary CTA; icon-only delete; tags; trash
│   └── SideBar.vue              # Categories; trash; tags; ContextMenu; ≤720px icon rail
├── SettingsWindow.vue           # Nav + section router; shortcut-recording window listener; ≤720px icon nav
│   └── settings/Settings*.vue   # 13 sections (shortcuts/appearance/features/source/history/tags/privacy/stats/data/sync/system/help/about)
│                                #   shared store access via composables/useSettings.ts; primitives in styles/settings.css
├── WelcomeDialog.vue            # First-run welcome (BaseDialog); onboarding_completed
├── BatchBar.vue                 # Shared batch actions (window)
├── ToggleSwitch.vue             # Shared switch primitive (settings sections)
├── TextInput.vue                # Shared single-line text input + trailing clear button (hidden when empty/disabled/readonly; clears + refocuses)
├── PasswordInput.vue            # Shared password input + show/hide toggle (keeps value/focus/caret; aria-label swaps 显示/隐藏密码)
├── BaseDialog.vue               # Teleport + Esc + focus trap; shared dialog chrome
├── ConfirmDialog.vue / TagDialog.vue / AliasDialog.vue  # Content slots on BaseDialog
├── ContextMenu.vue              # Fixed + clamp; Arrow/Enter/Esc; role=menu
├── WindowControls.vue
├── ToastHost.vue
├── TrayMenuApp.vue              # Custom tray-menu window entry (src root, Vite multi-page)
├── composables/useVirtualList.ts · useColumnResize.ts · useSettings.ts · useFeature.ts · useBatchActions.ts · useClipboardHotkeys.ts · useClipboardEvents.ts · useToast.ts · useConfirm.ts · useBatchBarHeight.ts · useExpireCountdown.ts · usePreviewActions.ts · usePreviewFormatting.ts · useRecordActions.ts · useSidebarMenus.ts · useTrayTheme.ts · useSearchHistory.ts · pasteFocusLock.ts
├── utils/mediaUrl.ts · sanitizeHtml.ts · trayMenuItems.ts · highlightSearch.ts · clipboardColor.ts · recordFormatting.ts · themeColors.ts · sourceBadge.ts
├── features/capabilities.ts     # FeatureId + DEFAULT_FEATURES (Rust: features.rs)
└── stores/clipboard.ts          # Orchestrator; fragments: clipboardList.ts · clipboardRecordActions.ts · clipboardTagActions.ts · clipboardExpiry.ts · settings.ts
```

### Backend (Rust) Module Layout
- `lib.rs` — `run()`: logging, dirs, DB init, plugin registration, `invoke_handler`, window events, resume safety-net
- `setup.rs` — one-time setup closure (capture pipeline, autostart, shortcut, tray, corners, backdrop, cleanup thread)
- `commands/` — Tauri commands: `mod.rs` (re-exports + `MAX_PAGE_SIZE`/`MAX_BATCH_IDS`), `records.rs`, `paste.rs`, `settings.rs`, `tags.rs`, `tray.rs`, `import_export.rs`, `search_history.rs`, `webdav.rs`
- `window.rs` — adaptive / remembered size, round corners, resize persistence. **Min width 760** (SideBar+List+Preview ≥740).
- `tray.rs` — tray icon (no native menu); right-click shows `tray-menu` window; left-click → `toggle_main_panel`; **Windows power-resume** rebuilds tray + reloads webviews
- `clipboard/` — `mod.rs` (monitor re-export), `monitor.rs` (Windows event loop + watchdog / non-Windows poll loop, sequence/fp watermark, suppression), `capture` lives in `capture.rs` (worker threads + periodic cleanup ~60s), `paste.rs` (target HWND, focus restore + Ctrl+V), `write.rs` (text/PNG/image write), `fgwin.rs` (foreground window), `image.rs` (image fingerprint/downscale ≤2560 edge)
- `capture.rs` — capture worker + **periodic cleanup thread** (~60s)
- `panel.rs` — `show_main_panel` / `toggle_main_panel`, `apply_global_shortcut`, adaptive size, `list_ipc_payload`
- `media.rs` — encode/store/load/delete (max edge **2560**, thumb **160**); media dir size cache
- `detect.rs` — content type + sensitive detection + SHA-256 helpers. Link type via `security::is_openable_link`
- `security.rs` — media path must resolve under media root; export/import JSON path checks; **openable-link whitelist** (`is_openable_link` / `link_scheme` + `LINK_PREFIXES`); DPAPI; safe media rel-paths only
- `features.rs` — feature flags (tags/batch/sync/stats) + `require_feature`
- `types.rs` — `ClipboardRecord` / `Settings` / `StatsData` / `TagInfo` / `SearchResult` / `RecordsPage` / `SearchHistoryEntry` / `AutoTagRule` + serde defaults
- `db/` — SQLite layer: `mod.rs` (constructor, read/write lock split); `types.rs` (`RECORD_COLS` / `RECORD_COLS_LIST`); `schema.rs` (FTS5 + schema version); `schema_tests.rs`; `records_query.rs` / `records_search.rs` / `records_write.rs` / `records_media.rs` / `records_import.rs`; `search_history.rs` (search-history autocomplete, **local-only**, cap 50, upsert count + recency); `settings.rs` (settings + DPAPI); `tags.rs` (tag CRUD + auto-tag); `stats.rs` (aggregates). **WAL:** write `conn` + **read pool** (3× `query_only`). Export: `get_records_for_export`.
- `webdav/` — WebDAV cloud sync (`client.rs` HTTP client; `sync.rs` pull/merge/push; `bundle.rs`; `media.rs`). Protocol `clipvault-webdav-v1`; manifest + JSONL bundle. **Tags sync with records**: bundle carries each record's tag names; `import_records_with_merge` merges links by tag name under a **tag LWW gate** (replace only when the incoming snapshot is strictly newer — `set_record_tags_by_name_conn`, non-empty-only additionally, returns changed-bool → `tags_changed` count) and tag mutations (`add/remove/set/rename`) bump `updated_at` so tag-only edits propagate via the record-level LWW watermark; an older snapshot can never roll back a newer local tag edit. Tag definitions merge by name (no cross-device id reuse); tag *deletion* does not tombstone. `WebDavSyncResult` is **structured counts only** (no `message`): `tags_pulled`/`tags_pushed` surface tag deltas; the frontend builds the success text via `utils/webdavResult.ts` (i18n clause composition, zero-count clauses omitted). Every run is logged into the **local `sync_history` table** (cap 50, never synced; commands `get_sync_history`/`clear_sync_history`, `sync`-feature gated; success stores counters, failure stores error text). Settings page: **Sync** (`SettingsSync.vue`) shows the live status + recent-sync list. Default remote dir `ClipVaultSync`.
- `main.rs` — `clipboard_lib::run()`

### State Management (Pinia)
- `clipboardStore` — records, category×tag AND filters, trash exclusive, batch, pause, pagination (60 / `has_more`), keyset/`listFetchOffset`, `listSort` (session), `ensureRecordDetail` for HTML; `loadRecords`/search re-fetches detail for current selection. Orchestrated in `stores/clipboard.ts`, with per-domain fragments (`clipboardList.ts` list/pagination/search · `clipboardRecordActions.ts` mutations · `clipboardTagActions.ts` tags · `clipboardExpiry.ts` expiry/stats scheduler) that late-bind their shared `Ref`s through typed context objects.
- `settingsStore` — debounced auto-save (200ms); theme / appearance; `always_on_top`; `features` capability flags (`tags`/`batch`/`sync`/`stats`, default all on); `enable_auto_tag` + `auto_tag_rules`; `onboarding_completed`; applies CSS vars + body class (`blur-enabled`) + `set_window_corner_radius`
- **Feature capabilities:** `settings.features` + `src/features/capabilities.ts` / Rust `features.rs`. Off → hide UI, skip capture hooks, reject related commands, keep data. Tags off also disables tag filter/search (`include_tags` on list/search SQL + FTS column filter).

### Key Design Decisions
- **Brand:** Product name **Clipboard** everywhere (title bar, about, `tauri.conf` window title). Version lives on the About page only.
- **First-run onboarding:** `WelcomeDialog` when `onboarding_completed` is false. New install Default=`false`; **upgrade** JSON missing the field deserializes to `true` (skip). Dismiss / Esc sets true and saves.
- **Single window:** One borderless, `transparent: true`, `shadow: false` window (`app_visible`). SideBar + `WindowControls` + list-toolbar; `mode_size_bounds` min width **760**. Shared `.panel-surface` chrome. **Size:** `resolve_panel_size` prefers last user resize (`window_*` in settings); if unset (0), falls back to `adaptive_panel_size`. Resize is debounced ~400ms into SQLite; maximized sizes are not saved. Frontend `save_settings` never overwrites size fields (`SIZE_SAVE_GEN`). **Always on top:** `settings.always_on_top` (default false) → Start/end flag applied via `apply_window_flags`. Paste with auto-close **minimizes** the window (never a separate floating panel).
- **List sort:** Toolbar `<select>` → `clipboardStore.listSort` → `get_records` / `search_records` `sort` param. Whitelist: `updated_desc` (default), `updated_asc`, `created_desc`, `copies_desc`. Non-trash: `is_pinned DESC` first. Session-only. `onNewRecord` prepends only for `updated_desc`; other sorts reload (debounced ~400ms).
- **True round corners (Windows):** CSS `border-radius` alone leaves black rectangular corners on transparent WebView2. Clip HWND with `SetWindowRgn` from `panel_radius` × DPI. Command: `set_window_corner_radius`.
- **Source label:** List + preview show the source app as plain text via `resolveSourceLabel` / `sourceBadge.ts` (empty →「系统剪贴板」). The preview meta line's 来源 item shows a tooltip with the raw exe path (`来源：记事本 (notepad.exe)`).
- **Theming / tokens:** CSS vars on `:root` (incl. `--type-*`, `--pin` / `--pin-soft`, `--text-xs`…`--text-xl`, `--space-*`, `--win-close-hover`). Themes: `.light-theme` / `.oled-theme` + six fixed colorful presets: dark `.dracula-theme` (紫夜) / `.nord-theme` (冰蓝) / `.sunset-theme` (暖橙) and light `.dracula-light-theme` (紫霞) / `.nord-light-theme` (冰白) / `.sunset-light-theme` (暖阳) + a hand-drawn pair `.handdrawn-theme` (手绘) / `.handdrawn-light-theme` (手绘·浅) + a hand-drawn pair `.handdrawn-theme` (手绘) / `.handdrawn-light-theme` (手绘·浅) + a monochrome pair `.mono-theme` (黑白) / `.mono-light-theme` (黑白·浅) + a colored-pencil pair `.pencil-theme` (彩铅) / `.pencil-light-theme` (彩铅·浅) — waxy colored-pencil hues on drawing paper with hatched/scribbled edges + a pixel pair `.pixel-theme` (像素) / `.pixel-light-theme` (像素·浅) — NES-style chunky pixel surfaces with hard offset block shadows and pixel-art sprite icons. `settings.theme` union: `dark|light|oled|dracula|nord|sunset|dracula-light|nord-light|sunset-light|handdrawn|handdrawn-light|mono|mono-light|pencil|pencil-light|editorial|editorial-light|sticker|sticker-light|flat|flat-light|pixel|pixel-light`; Rust stores it as a plain `String` (no validation, no migration). Colorful themes are **fixed presets** (each a full ~30-token block, either dark- or light-leaning). Settings UI lists all 23 theme cards **flat in a single radiogroup** (no sub-groups — grouping implies a combinable lightness×hue axis that does not exist; ADR-0003). Theme switching lives only in Settings → Appearance; the sidebar quick menu has no theme toggle. A legacy saved `theme: "system"` value (the removed follow-system option) normalizes to `dark` on load (ADR-0004). **Theme files live per-family under `src/styles/themes/`** (`base.css` / `dracula.css` / `nord.css` / `sunset.css` / `handdrawn.css` / `mono.css` / `editorial.css` / `sticker.css` / `flat.css` / `pencil.css` / `pixel.css`), imported at the top of `main.css`; `:root` default + global rules stay in `main.css`. The hand-drawn family is the only one with **extra sketch styling** (hand-drawn icons — AppIcon swaps Lucide for `@sketchyicons/vue` under `body.handdrawn-*` via the `useHanddrawnTheme` composable — wobbly `--sketch-radius` on cards/buttons/tabs/inputs, sticker hard shadows, card tilts, wavy marker underlines + SVG squiggle dividers replacing crisp hairlines, paper-dot texture, dashed focus rings, marker-yellow search highlight/selection, italic placeholders) gated on `body.handdrawn-*`; the pixel family swaps clean Lucide icons for its own **pixel-art sprite set** (`PIXEL_ICONS` in `src/components/icons/pixelIcons.ts`, chosen under `body.pixel-*` via the `usePixelTheme` composable; a partial set — unmapped names fall back to Lucide) plus hard offset pixel shadows; the monochrome family is pure grayscale tokens (semantic colors differentiated by lightness) plus a tiny component-fix block (ink text on the white-accent primary buttons, ink toggle knob, B&W selection/search highlight) — adding a family is still "token block + THEME_CLASSES entry + settings card + i18n" (ADR-0003), plus optionally a shared `body.`-prefixed visual override block in its own file.
  - **Accent:** Fluent blue `--accent: #0078d4` (dark + light). Hover/light variants: dark `#1b86d9` / `#60cdff`; light hover `#106ebe`. Focus rings / primary CTA /「全部」nav use accent.
  - **Column surfaces:** SideBar `--bg-elevated`; list + preview share `--bg-surface` (content band). Separated by a single list `border-right`.
  - **Type colors (`--type-*`):** text sky `#7dd3fc` · code green `#34d399` · link deep blue (dark `#60a5fa` for AA contrast / light `#2563eb`) · image cyan `#0ea5e9` · file amber `#eab308`. Badges (`.badge-*`), SideBar category active (`--cat-color`), and type icons / link titles follow these. List **selection/hover** is Fluent flat (accent soft fill / `--bg-hover`), not type-colored card borders.
  - **Pin vs favorite:** `--pin` violet (dark `#a78bfa` / light `#7c3aed`) for pinned UI — not red, so it stays distinct from `--danger`; `--warning` gold for favorites. Preview bottom bar uses `action-pinned` vs `action-active` — do not share one “active” style for both.
  - **Pinned list chrome:** 「置顶」section label (pin color) + hairline divider before the first unpinned row when both groups exist (virtual-list `divider` item).
  - **Tag palette:** Fixed 12-color hue wheel in `themeColors.ts` / `db/tags.rs` (`TAG_PALETTE_HEX`); no free-form picker. Off-palette SQLite hex snapped once at startup (`tag_palette_v2`).
- **Blur:** Setting `enable_blur` defaults **false**. Frosted glass comes from the **native DWM acrylic** backdrop (`set_window_backdrop` → `Effect::Acrylic`; Win11 `DWMSBT_TRANSIENTWINDOW`, Win10 `ACCENT_ENABLE_ACRYLICBLURBEHIND`) — CSS `backdrop-filter` cannot blur the OS desktop behind a transparent WebView2. When on, `body.blur-enabled` also makes `.panel-surface` (and tray-menu) backgrounds translucent so the blurred desktop shows through. Intensity adjustable via `blur_strength` (30–80%, default 45): surface tint opacity = `100 − blur_strength` (CSS var `--panel-blur-opacity`).
- **Custom tray menu:** Separate `tray-menu` WebView (Vite multi-page). Right-click anchors above tray icon; theme/blur follow settings. **Left-click** → `toggle_main_panel`: hidden/minimized → show + focus; visible but not foreground → bring to front (`show_main_panel` / `focus_window`); already foreground → hide. After sleep/wake, power watcher rebuilds tray + reloads webviews.
- **Font size:** Root `font-size` = setting (default **16px**). Rem baseline is **16px** (`--ui-font-scale = font_size/16`). Prefer `rem` / `--text-*` so Settings / dialogs scale with the user preference. Virtual list row height scales with `font_size`.
- **Font family:** `settings.font_family` (default `'default'`) is a preset key (`default`/`yahei`/`simhei`/`simsun`/`kaiti`/`segoe` from `src/utils/fontPresets.ts`) or `system:<name>` for an OS-installed font. `applyAppearance` sets `--font-sans` via `resolveFontStack` (every stack carries a CJK-capable fallback — `Microsoft YaHei UI` on Windows). System fonts come from the **async** Rust `get_system_fonts` command (`commands/fonts.rs`): DirectWrite via `font-kit`, filtered to families containing a CJK glyph (`glyph_for_char('\u{4E00}')`), common-first then alphabetical, enumerated on a background thread (`tauri::async_runtime::spawn_blocking`, no UI freeze) and cached in a static `Mutex`. Settings stored as a JSON blob → new field needs no DB migration (serde default).
- **Responsive (window):** `@media (max-width: 720px)` — SideBar / settings nav → icon rail (sidebar resizer hidden); preview actions denser grid; theme cards 2×2.
- **Column resize:** SideBar width and list-column width are user-draggable (`useColumnResize` composable: pointer events + rAF throttle). Widths persist in localStorage (`clipboard-sidebar-width`, `clipboard-list-col-width`). List column always uses its stored width (no jump on preview open/close); first run captures the natural flex width via DOM measurement. Sidebar resize disabled ≤720px (icon-rail mode).
- **Motion / animation:** Follow `docs/Clipboard-交互动效规范.md`. **Never `transition: all`** — always an explicit property list. Prefer compositor-friendly props (`opacity` / `transform`); don't continuously animate layout (`padding`/`margin`/`height`/`grid-template-rows`) or `background` paint. All durations come from `--transition-*` tokens — no hardcoded `0.15s` etc. BatchBar floats **absolute** over the list (`batch-bar-holder`, main.css) so toggling batch mode never reflows the list; hosts reserve its height via `useBatchBarHeight` (ResizeObserver) as transitioned top `padding`. Column resizers (`div.resizer`) also overlay (`margin-left: -4px` + `z-index: 10`) instead of reserving flex space. New-record flash animates `opacity` on a `::before` overlay, not `background`. Loading/empty ↔ list fade-in is a **pure CSS animation** on `.list-body--enter` (`opacity` + `translateY(-8px)`, `--transition-smooth`) — **not** a JS `<Transition mode="out-in">`: WebView2 drops `requestAnimationFrame` while its host window is hidden, so an out-in leave stalls forever and leaves the list unmounted (blank list on cold start); CSS animations resume/complete on their own and never gate mounting. Respects `anim-disabled` / `prefers-reduced-motion`.
- **A11y (baseline):** Record list `role="listbox"` / `option` + roving tabindex; dialogs via `BaseDialog` (Esc + focus trap); `ContextMenu` keyboard + clamp; global `:focus-visible`; theme cards `role="radio"`; form `aria-label`s on search / ranges / ignore-app input. Tertiary text colors raised for WCAG-ish contrast.
- **Preview actions:** 「粘贴」is `action-primary` (solid accent); delete is icon-only. Pin and favorite are on the bottom action bar / hotkey / context menu / list row (not in preview header).
- **Sensitive detection** (text only): `password|passwd|pwd`; 4–8 digits + `验证码|code|Code`; `sk-`+≥20 alnum; 16–19 digits with len≤25. Default expire 600s. `is_sensitive` is a **bool**, not a `content_type` (ContentType = text|code|link|image|file only).
- **Single-record text cap:** `max_text_bytes` (bytes, default 10 MB, `0` = unlimited) skips oversized text copies entirely — never stored, so the DB + FTS trigram index can't be bloated and the write lock never stalls on a giant index build. The OS clipboard itself is untouched. Enforced in `capture.rs::process_text_job` and `win_history.rs::import_text`; surfaced in Settings → 历史.
- **Link / download URI detection:** Whole trimmed clipboard string only (no mid-caption extract). Whitelist schemes → `content_type: link` (no new type): `http`/`https`/`ftp`/`magnet`/`ed2k`/`thunder` (case-insensitive). Bare `magnet:` / `http://` rejected. `javascript:`/`data:`/`file:` never links. Default auto-tag「链接」applies. **Open:** `open_url` → `ShellExecuteW` for any whitelisted scheme (browser or installed protocol handler). **Preview:** http(s) still `<a target="_blank">`; magnet/ed2k/thunder/ftp click → `invoke('open_url')` (label `preview.downloadLink`). **Import:** keep `link` only if `is_openable_link` (not http(s)-only).
- **Color swatch (not a type):** If plain `text` content is a standalone CSS color (`#rgb` / `#rrggbb` / `rgb()` / `hsl()`, whole string), list shows a swatch chip and preview shows a large swatch — still `content_type: text`.
- **Soft delete:** Delete → trash (toast, no confirm). Permanent delete / empty trash still confirm.
- **Memory (frontend):** List soft-capped (`PAGE_SIZE * 2`) on `onNewRecord` / `loadMore`. Full content/HTML in `recordDetails` (max ~6). Batch copy fetches full text via `get_records_by_ids` (one IN query; list rows are truncated).
- **Clipboard fingerprint:** SHA-256 of text+html (not retaining full HTML string in `last_text_fp`). Image read path: quick-fp only; worker computes full hash.
- **Retention “回收站保留天数”:** Only purges trashed rows. **最大记录数** evicts oldest non-favorite / non-pinned when inserting (write lock).
- **Toast policy:** Actions without clear UI state (paste, trash, errors). Not for pin/favorite/settings toggles. Failed tag create/assign must toast error. Host: top-right (`top: 60px`) so toasts clear the title-bar controls.
- **Rich text:** Capture CF_HTML → `content_html`. List/search omit HTML; preview loads via `get_record` / detail. Preview uses **DOMPurify + `v-html`**. Paste writes original HTML back.
- **Preview chrome:** Type + meta in header (no pin/favorite buttons); source / time / size-or-chars / 富文本 / 粘贴次数 as one meta line. **Borders:** header bottom divider only (no `preview-actions` / `sidebar-bottom` top rules). Text body has no box border; link/file use elevated fill without stroke; image thumb uses a hairline outline for contrast. **Spacing:** `.preview-tags` `8px 20px 16px`; `.preview-actions` `8px 20px 20px` (horizontal 20px aligns with content). Image preview: click → `open_record_media` (canonicalize under media root; Windows `ShellExecuteW` — not `cmd /c start` / `shell.open`). **Link preview:** any openable link (http(s)/ftp/magnet/ed2k/thunder) click → `open_url` (OS default handler). http(s) still renders as an `<a href>` so right-click copy works, but the click is intercepted (`@click.prevent`) — `target="_blank"` navigation is unreliable/blank in WebView2; download schemes render as a styled button.
- **Filters:** Type/favorites **AND** tag combine; trash is exclusive. IPC: `get_records` / `search_records` / `get_all_tags` use `rename_all = "snake_case"`. Tag counts follow active category. SideBar: zero-count tags fold under「更多」; `is_auto` tags show a sparkles icon + tooltip「自动打标规则创建」(active zero-count tag stays in the primary list).
- **Record alias:** Optional short `alias` (max 80 chars) for display only — does **not** change paste content / hash / HTML. List title prefers alias (hover `title` = content preview). Edit via preview header or context menu (`set_record_alias`). Hash-dedup re-copy keeps existing alias. Import/export include `alias` (serde default `""`). Alias edits bump `updated_at` (only when the alias actually changed) so alias-only edits propagate through the record-level WebDAV merge — same sync-watermark rule as tag edits.
- **Auto-tag:** Settings `enable_auto_tag` (default **true**) + `auto_tag_rules`. Per-rule match is OR. No per-tag FTS triggers — refresh FTS once after batch tag writes (**FTS v5**). Defaults: 链接←`link`; 部署 / 前端←keywords. UI: Settings → 标签 (local draft + 400ms commit). `scheduleLoadTags` 350ms.
- **Search:** FTS5 trigram (**≥3 chars**) on content / alias / source_app / source_window / tags; `content` is indexed from its **first 32K chars** (trigram size bound, FTS v5) and the FTS candidate list is capped at 10k (rank-ordered) before the outer keyset sort. **1–2 chars:** single-pass `instr(...)` + tag `EXISTS` (no `LIKE '%X%'`); **1-char queries skip the content column** (alias/source/tags only). FTS update trigger is **`OF content` only** so hash-dedup source updates do not rebuild FTS. Tag / alias changes call `refresh_record_fts`. **FTS delete:** `DELETE FROM records_fts WHERE rowid=…` (not FTS5 `'delete'` command — broken on Windows SQLite).
- **Search history / autocomplete:** `SearchBar` dropdown of recent searches (top **10**). Stored in the `search_history` table (`query` PK, `search_count`, `last_searched_at`) — **local-only**, never exported/synced; DB cap **50**. Recorded only on deliberate submit (Enter / suggestion-select), never on debounced intermediate typing. Frontend `useSearchHistory` loads once on mount and writes optimistically (fire-and-forget invoke; failed writes silently dropped). Keyboard: ↑/↓ navigate, Enter fills+searches, Delete removes the active row, bottom「清空历史搜索」clears all.
- **Stats storage:** `storage_bytes` ≈ `SUM(content_len)` (+ HTML lengths) + cached `media/` dir size. `data_path` is the absolute app data dir; displayed on the **Data** settings page (moved from Stats).
- **Sets in Vue:** Never mutate `Set` in place — assign a new `Set`.
- **Global shortcut:** From `settings.global_shortcut` at startup; re-bound in `save_settings`.
- **Pause capture:** Frontend + tray both update Rust; tray emits `capture-paused`.
- **Cleanup:** Independent background thread (~60s): `cleanup_expired` + `cleanup_retention`. Not on the capture hot path. Frontend expire sweep + `records-expired` event sync the list.
- **File type detect:** Path heuristic only (no `Path::exists` on monitor thread).
- **Dedup:** SHA-256 of text fingerprint (plain+html) or full image bytes. Check + update/insert under the **same write Mutex**. Hash match updates `updated_at` / source (active rows only) — does **not** bump `copy_count`. `copy_count` starts at **0** and increments only on paste from Clipboard. **`updated_at` semantics = content freshness only:** capture / re-copy (dedup) / tag edits (sync watermark, `touch_record_updated_at`) bump it; **paste does not** (just `copy_count`), so pasting never re-ranks `updated_desc`, protects from capacity eviction, or raises the WebDAV LWW watermark. Alias edits also bump it (same sync-watermark rule — see `set_record_alias`). The frontend mirrors tag/alias-edit bumps instantly via `reorderForUpdates`.
- **Source app:** Foreground process via `QueryFullProcessImageNameW` + `PROCESS_QUERY_LIMITED_INFORMATION` (not `GetModuleFileNameW`, which only works for the current process). Empty `source_app` falls back to UI label「系统剪贴板」.
- **Paste self-write:** `paste_record` suppresses monitor emits ~1.5s. While suppressed, the monitor skips all reads **and does not commit the sequence watermark** (nor `last_text_fp` / `last_image_hash`) — otherwise a real copy in that window is permanently lost. The first pass after the window re-reads the current clipboard; our own paste is then absorbed by DB hash-dedup.
- **Paste focus:** On panel show, remember previous foreground HWND. Paste writes clipboard (PNG bytes preferred for images), focuses target while still holding FG, then minimize window when auto-close, Ctrl+V. No valid target → clipboard only. `auto_close_on_paste` false → restore panel (unminimize/show) without stealing focus.
- **Hide-on-close / single instance / autostart:** tray minimize, single-instance focus, OS Run-key sync.
- **WebView noise:** `Chrome_WidgetWin_0` Error 1412 on exit is harmless.
- **Architecture decisions:** [`docs/adr/`](docs/adr/) — ADR-0001 covers the virtual-list composable extraction and the responsive grid-column single-source-of-truth rule; ADR-0002 covers the (now removed) native OS-theme watcher behind "follow system" (superseded by ADR-0004); ADR-0003 covers the additive colorful preset themes; ADR-0004 covers the removal of the "follow system" theme; ADR-0005 covers the event-driven clipboard monitor (AddClipboardFormatListener + watchdog).

## Agent skills

### Issue tracker

Issues live in this repo's GitHub Issues (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical roles map 1:1 to tracker labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.
