// ============================================================
// ClipVault — TypeScript Types
// NOTE: Keep in sync with Rust structs in src-tauri/src/lib.rs:
//   ClipboardRecord, Settings, StatsData, TagInfo, SearchResult
// ============================================================

export type ContentType = 'text' | 'code' | 'link' | 'image' | 'file' | 'sensitive';

export interface ClipboardRecord {
  id: number;
  content: string;
  content_type: ContentType;
  source_app: string;
  source_window: string;
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
  /** Preview-specific fields (not stored) */
  preview?: string; // truncated content for list
  display_time?: string; // relative time string
}

export interface Tag {
  id: number;
  name: string;
  color: string;
  is_auto: boolean;
  count: number;
}

export interface Settings {
  // Shortcuts
  global_shortcut: string;
  // History
  max_records: number;
  retention_days: number;
  // Appearance
  theme: 'dark' | 'light' | 'oled' | 'system';
  panel_opacity: number;
  panel_radius: number;
  enable_blur: boolean;
  enable_animation: boolean;
  font_size: number;
  // Behavior
  app_mode: 'floating' | 'window';
  default_paste_mode: 'original' | 'plain' | 'markdown';
  auto_close_on_paste: boolean;
  // Privacy
  enable_sensitive_detection: boolean;
  sensitive_auto_expire_seconds: number;
  // Storage
  data_path: string;
  // System
  auto_start: boolean;
  minimize_to_tray: boolean;
  // Ignore apps
  ignored_apps: string[];
  /** Remembered logical window size (0 = adaptive). */
  floating_width: number;
  floating_height: number;
  window_width: number;
  window_height: number;
}

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
  type_distribution: Record<ContentType, number>;
}
