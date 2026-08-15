// ============================================================
// Clipboard — TypeScript Types
// NOTE: Keep in sync with Rust structs in src-tauri/src/types.rs:
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
  /** Device that first captured this record (empty = legacy/unknown origin). */
  source_device_id?: string;
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
  /** Tag name → palette-color pairs carried by the sync/export bundle (export only). */
  tag_colors?: [string, string][];
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
  ai: boolean;
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
  /** Search bar display mode. */
  search_mode: 'full' | 'icon' | 'hidden';
  /** How the record preview sits next to the list. */
  preview_layout: 'columns' | 'on_demand' | 'drawer';
  // Behavior
  always_on_top: boolean;
  default_paste_mode: 'original' | 'plain';
  auto_close_on_paste: boolean;
  // Privacy
  enable_sensitive_detection: boolean;
  sensitive_auto_expire_seconds: number;
  /** Skip text captures larger than this (bytes); 0 = unlimited. */
  max_text_bytes: number;
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
  /** Display name for this device, published in the sync manifest. */
  webdav_device_name: string;
  /** device_id → display name learned from sync manifests (local cache). */
  webdav_device_names: Record<string, string>;
  webdav_last_sync_at: string | null;
  /** AI enrichment (OpenAI-compatible chat completions). */
  enable_ai: boolean;
  ai_base_url: string;
  /** DPAPI-encrypted at rest; kept as-is in the running settings object. */
  ai_api_key: string;
  ai_model: string;
  /** Write the AI summary into the record alias (default true). */
  ai_summary_alias: boolean;
  /** Let the AI append auto-tags to records (default true). */
  ai_auto_tag: boolean;
  /** Content truncation before it leaves the machine (chars). */
  ai_max_chars: number;
  ai_min_chars: number;
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
 * IMPORTANT: Keep in sync with `default_auto_tag_rules()` in src-tauri/src/types.rs.
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
