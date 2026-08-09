// ============================================================
// Clipboard — TypeScript Types
// NOTE: Keep in sync with Rust structs in src-tauri/src/lib.rs:
//   ClipboardRecord, Settings, StatsData, TagInfo, SearchResult
// ============================================================

import type { ThemeKey } from "./utils/themeRegistry";

export type ContentType = 'text' | 'code' | 'link' | 'image' | 'file';
export type FilterTab = 'all' | 'text' | 'code' | 'link' | 'image' | 'file' | 'favorites';

export interface ClipboardRecord {
  id: number;
  content: string;
  content_type: ContentType;
  source_app: string;
  source_window: string;
  /** Friendly source name from the exe's FileDescription (display only; empty = fall back). */
  source_name?: string;
  hash: string;
  copy_count: number;
  is_favorite: boolean;
  is_pinned: boolean;
  is_sensitive: boolean;
  is_trashed: boolean;
  auto_expire_at: string | null; // ISO timestamp
  created_at: string;
  updated_at: string;
  tags: string[];
  /** HTML fragment when rich format was captured */
  content_html?: string | null;
  /** Relative path under app data dir (image records) */
  media_path?: string | null;
  thumb_path?: string | null;
  width?: number | null;
  height?: number | null;
  /** Absolute filesystem paths for convertFileSrc */
  media_abs?: string | null;
  thumb_abs?: string | null;
  /** Full content length (list rows may truncate `content`) */
  content_len?: number | null;
  /** Short display alias; empty = none. Does not change paste content. */
  alias?: string;
}

export interface Tag {
  id: number;
  name: string;
  color: string;
  is_auto: boolean;
  count: number;
}

export interface AutoTagRule {
  tag_name: string;
  keywords: string[];
  content_types: string[];
}

/** User-defined source display-name override (matches exe basename). */
export interface SourceNameOverride {
  exe_name: string;
  display_name: string;
}

/** Optional product capabilities — keep in sync with Rust `FeatureFlags`. */
export interface FeatureFlags {
  tags: boolean;
  batch: boolean;
  sync: boolean;
  stats: boolean;
}

export interface Settings {
  // Shortcuts
  global_shortcut: string;
  // History
  max_records: number;
  retention_days: number;
  // Appearance
  theme: ThemeKey;
  panel_opacity: number;
  panel_radius: number;
  enable_blur: boolean;
  blur_strength: number;
  enable_animation: boolean;
  font_size: number;
  /** UI font-family preset key, or `system:<name>` for an OS-installed font. */
  font_family: string;
  // Behavior
  app_mode: 'floating' | 'window';
  default_paste_mode: 'original' | 'plain';
  auto_close_on_paste: boolean;
  // Privacy
  enable_sensitive_detection: boolean;
  sensitive_auto_expire_seconds: number;
  /** Import the OS clipboard history (Win+V) once on startup (default off). */
  import_system_history_on_start: boolean;
  // System
  auto_start: boolean;
  minimize_to_tray: boolean;
  // Ignore apps
  ignored_apps: string[];
  /** User-defined exe → display-name overrides (frontend resolution). */
  source_name_overrides: SourceNameOverride[];
  /** Remembered logical window size (0 = adaptive). */
  floating_width: number;
  floating_height: number;
  window_width: number;
  window_height: number;
  /** Auto-tag new records from rules (default on). */
  enable_auto_tag: boolean;
  auto_tag_rules: AutoTagRule[];
  /** False until first-run welcome is dismissed. */
  onboarding_completed: boolean;
  /** UI language preference. */
  language: 'zh-CN' | 'en-US' | 'system';
  /** WebDAV sync (local credentials; not included in JSON export). */
  webdav_url: string;
  webdav_username: string;
  webdav_password: string;
  webdav_remote_path: string;
  webdav_sync_sensitive: boolean;
  webdav_device_id: string;
  webdav_last_sync_at: string | null;
  /** Optional modules; missing keys default true on load. */
  features: FeatureFlags;
}

export interface WebDavSyncResult {
  pulled: number;
  pushed: number;
  merged: number;
  tags_pulled: number;
  tags_pushed: number;
  media_downloaded: number;
  media_uploaded: number;
  media_skipped: number;
}

/** One WebDAV sync operation log row (local-only, never synced). */
export interface SyncHistoryEntry {
  id: number;
  synced_at: string;
  /** "pull" | "push" | "sync" */
  action: string;
  success: boolean;
  pulled: number;
  pushed: number;
  merged: number;
  tags_pulled: number;
  tags_pushed: number;
  media_downloaded: number;
  media_uploaded: number;
  media_skipped: number;
  error?: string | null;
}

/**
 * L-3: Default auto-tag rules shown in settings UI on reset.
 * IMPORTANT: Keep in sync with `default_auto_tag_rules()` in src-tauri/src/lib.rs.
 */
export const DEFAULT_AUTO_TAG_RULES: AutoTagRule[] = [
  { tag_name: "链接", keywords: [], content_types: ["link"] },
  {
    tag_name: "部署",
    keywords: ["deploy", "kubectl", "docker", "helm", "k8s", "npm run build", "生产环境"],
    content_types: [],
  },
  {
    tag_name: "前端",
    keywords: ["vue", "react", "typescript", "tsx", "vite", "webpack", "frontend", "前端"],
    content_types: [],
  },
];

export interface SearchResult {
  records: ClipboardRecord[];
  total: number;
  query: string;
  elapsed_ms: number;
  has_more: boolean;
}

export interface RecordsPage {
  records: ClipboardRecord[];
  has_more: boolean;
}

export interface SearchHistoryEntry {
  query: string;
  search_count: number;
  last_searched_at: string;
}

export interface ExportOptions {
  format: 'json' | 'csv' | 'markdown' | 'sqlite';
  include_images: boolean;
  exclude_sensitive: boolean;
  date_from?: string;
  date_to?: string;
  record_ids?: number[];
}

export interface ImportResult {
  total: number;
  imported: number;
  skipped: number;
  errors: string[];
}

export interface StatsData {
  total_records: number;
  total_copies: number;
  favorites_count: number;
  pinned_count: number;
  sensitive_count: number;
  storage_bytes: number;
  /** Absolute path to app data directory (DB + media). */
  data_path: string;
  type_distribution: Record<string, number>;
}
