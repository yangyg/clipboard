# CONTEXT.md — Clipboard

Clipboard is a **Tauri v2** desktop clipboard manager for Windows. It monitors the OS clipboard, persists text and image records in a local SQLite database, and provides a floating panel or window UI for browsing, searching, tagging, pasting, and managing clipboard history.

## Domain Glossary

| Term | Meaning |
|---|---|
| **Record** | A single clipboard capture entry — text (with optional rich HTML), image, link, code snippet, or file reference. Stored in SQLite with metadata (source app, timestamp, tags, copy count). |
| **Category** | Top-level filter: 全部 (All), 文本 (Text), 代码 (Code), 链接 (Link), 图片 (Image), 文件 (File). Derived from `content_type`. |
| **Openable link** | Whole-string clipboard URI whose scheme is in the whitelist (`http`/`https`/`ftp`/`magnet`/`ed2k`/`thunder`). Classified as `content_type: link`; opened via `open_url` → OS handler (`ShellExecuteW`). Not a separate content type. |
| **Tag** | User-assigned or auto-assigned label on records. Tags support AND filtering with categories. |
| **Auto-tag** | Rules (`auto_tag_rules`) that match record content type or keywords and assign tags automatically on insert. |
| **Soft delete / Trash** | Deleted records move to trash first (recoverable). Permanent delete and empty-trash require confirmation. |
| **Sensitive record** | Records matching patterns (passwords, verification codes, API keys, credit-card-like numbers). Auto-expire after a short TTL (default 600s). `is_sensitive` is a bool, not a content type. |
| **Hash dedup** | SHA-256 of text fingerprint (plain + HTML) or full image bytes. Duplicate copies update `updated_at` only — do **not** bump `copy_count`. |
| **Floating panel** | Always-on-top compact overlay. Hides on blur. Kept mounted via `v-show`. |
| **Window mode** | Full window with SideBar + RecordList + PreviewPane. Min width 760px. |
| **Paste target** | The foreground HWND at the moment the panel opened. Paste writes clipboard → focuses target → sends Ctrl+V. |
| **Source app** | The executable name of the process that owned the clipboard content at capture time. Shown as a plain-text label via `resolveSourceLabel` (friendly name, empty →「系统剪贴板」); the preview meta line's tooltip shows the raw exe path. |
| **Keyset pagination** | List queries use keyset cursors (`before_pinned` / `before_updated_at` / `before_id`) instead of OFFSET to avoid drift when new rows prepend. |
| **Soft cap** | In-memory list pages are soft-capped (`PAGE_SIZE × 2`). When dirty, the next `loadMore` reloads from DB. |
| **WebDAV sync** | Cloud sync via WebDAV protocol `clipvault-webdav-v1`. Manifest + JSONL bundle; media files synced alongside. Default remote dir `ClipVaultSync`. |
| **Tombstone** | Deletion marker `(hash, deleted_at)` published in the WebDAV manifest (manifest version 2). Explicit deletions propagate cross-device: recipients move their older copies to trash (recoverable); a strictly newer re-copy wins and supersedes the tombstone. Automatic cleanup (eviction / sensitive expiry) never writes tombstones. |
| **Search history** | Distinct search terms submitted via Enter / suggestion-select, stored in the `search_history` table (query PK + count + last_searched_at). Drives the search-box autocomplete dropdown (top 10, recency-ordered). **Local-only** — excluded from export/import and WebDAV sync. |
| **UI font** | `font_family` setting — a preset key (`default`/`yahei`/`simhei`/`simsun`/`kaiti`/`segoe`) or `system:<name>` for an OS-installed font. Applied as `--font-sans`; every stack carries a CJK-capable fallback (`Microsoft YaHei UI`). |

## Architecture Decision Records

See `docs/adr/` for immutable decision records:

- **ADR-0001** — Virtual-list composable extraction & responsive grid column single-source-of-truth (JS, not CSS `auto-fill`).
- **ADR-0002** — Native OS-theme watcher (invisible HWND + `WM_SETTINGCHANGE`) as the primary source for follow-system theme, because WebView2 matchMedia events are unreliable while hidden. **Superseded by ADR-0004 (feature removed).**
- **ADR-0003** — Colorful preset themes are additive fixed full-token blocks (dark `dracula`/`nord`/`sunset` + light `dracula-light`/`nord-light`/`sunset-light`), extending the `theme` union; no custom accent / `color-mix` refactor. Later appended: per-family token files under `src/styles/themes/`, the hand-drawn family (`handdrawn`/`handdrawn-light`, with sketch styling + `@sketchyicons/vue` icons) and the monochrome family (`mono`/`mono-light`).
- **ADR-0004** — Removed the "follow system" theme option entirely (supersedes ADR-0002). Legacy saved `theme: "system"` normalizes to `dark` on load; the theme UI is 13 fixed cards in one radiogroup.

## Key Design Constraints

- **Single JS source of truth for grid columns** — `gridCols` from ResizeObserver, applied as inline `grid-template-columns`. Never CSS `auto-fill` (would drift from virtualizer row grouping).
- **Text-first clipboard reading** — Meaningful share text skips `get_image()`. Only call it when bitmap/DIB is reported.
- **Paste self-write suppression** — After paste, monitor skips reads for ~1.5s and does **not** advance sequence watermark or fingerprints, so the next real copy is not lost.
- **FTS update is `OF content` only** — Hash-dedup source updates do not rebuild FTS. Tag/alias changes call `refresh_record_fts` explicitly.
- **Loading/empty ↔ list transition is pure CSS** — Not `<Transition mode="out-in">`, because WebView2 drops `requestAnimationFrame` while hidden.
- **Media open uses `ShellExecuteW`** — Not `cmd /c start` or `shell.open`.
- **Link schemes are a shared whitelist** — `security::is_openable_link` is the single source for detect / `open_url` / import keep-as-link. WebView `<a href>` stays http(s)-only; other openable schemes go through Rust.
- **UI font via presets / system fonts** — `font_family` resolves through `src/utils/fontPresets.ts` (`resolveFontStack`); system-font choices (`system:<name>`) are enumerated by the async Rust command `get_system_fonts` (DirectWrite via `font-kit`, CJK-glyph filtered, cached) and applied with a CJK-safe fallback stack. Settings is a JSON blob, so new fields need no DB migration (serde default).
- **Search history is local-only** — `search_history` rows never enter export/import (`get_records_for_export`) or WebDAV bundles, keeping the blast radius of search terms identical to the old localStorage store (just durable). Recorded only on deliberate submit (Enter / suggestion-select), never on debounced intermediate typing.
- **Brand name** — Product name is **Clipboard** everywhere in UI. Machine-readable names use `clipboard`. Compatibility identifiers (bundle ID, data dir, DB filename) retain legacy `ClipVault` names.

## Data Paths

- Database: `%LOCALAPPDATA%/ClipVault/clipvault.db`
- Media: `%LOCALAPPDATA%/ClipVault/media/`
- Log: `%LOCALAPPDATA%/ClipVault/logs/clipvault.log`

## Motion & Animation

See `docs/Clipboard-交互动效规范.md` for the animation design spec. Key rule: never `transition: all` — always explicit property list. Prefer compositor-friendly props (`opacity` / `transform`).
