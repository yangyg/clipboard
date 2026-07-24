// ============================================================
// ClipVault — TypeScript Types
// NOTE: Keep in sync with Rust structs in src-tauri/src/lib.rs:
//   ClipboardRecord, Settings, StatsData, TagInfo, SearchResult
// ============================================================

export type ContentType = 'text' | 'code' | 'link' | 'image' | 'file';

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

export interface AutoTagRule {
  tag_name: string;
  keywords: string[];
  content_types: string[];
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
  /** Auto-tag new records from rules (default on). */
  enable_auto_tag: boolean;
  auto_tag_rules: AutoTagRule[];
  /** False until first-run welcome is dismissed. */
  onboarding_completed: boolean;
}

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
