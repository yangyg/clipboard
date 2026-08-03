/**
 * Pure list-row display helpers extracted from RecordList.vue so the SFC
 * script stays under 200 lines. Every function is deterministic on its args;
 * `t` is the vue-i18n translate function (kept as a parameter so this stays a
 * dependency-free util module).
 */
import { parseClipboardColor } from "./clipboardColor";
import { sourceShortName } from "./sourceBadge";
import { escapeHtml, highlightedPreview, highlightSearchHtml } from "./highlightSearch";
import type { ClipboardRecord } from "../types";

export type TranslateFn = (key: string, named?: Record<string, unknown>) => string;

const PREVIEW_MAX_LEN = 80;

export function recordAlias(record: ClipboardRecord): string {
  return (record.alias ?? "").trim();
}

export function contentPreview(record: ClipboardRecord, t: TranslateFn): string {
  if (record.content_type === "image") {
    if (record.width && record.height) {
      return t('record.imageLabel', { w: record.width, h: record.height });
    }
    return t('record.imageOnly');
  }
  if (record.content.length <= PREVIEW_MAX_LEN) return record.content;
  return record.content.slice(0, PREVIEW_MAX_LEN) + "…";
}

/** List primary line: alias when set, otherwise content preview. */
export function getPreview(record: ClipboardRecord, t: TranslateFn): string {
  const alias = recordAlias(record);
  if (alias) return alias.length > PREVIEW_MAX_LEN ? alias.slice(0, PREVIEW_MAX_LEN) + "…" : alias;
  return contentPreview(record, t);
}

/** Hover shows original content when an alias is displayed. */
export function recordTitleAttr(record: ClipboardRecord, t: TranslateFn): string | undefined {
  if (!recordAlias(record)) return undefined;
  return contentPreview(record, t);
}

/** Safe HTML for list title — highlights search hits when querying. */
export function previewHtml(record: ClipboardRecord, query: string, t: TranslateFn): string {
  const alias = recordAlias(record);
  const q = query.trim();
  if (alias) {
    if (!q) return escapeHtml(getPreview(record, t));
    return highlightedPreview(alias, q, PREVIEW_MAX_LEN);
  }
  if (record.content_type === "image") {
    return escapeHtml(getPreview(record, t));
  }
  if (!q) return escapeHtml(getPreview(record, t));
  return highlightedPreview(record.content, q, PREVIEW_MAX_LEN);
}

/** Text that is only a CSS color → list swatch instead of type icon. */
export function rowColor(record: ClipboardRecord): string | null {
  if (record.content_type !== "text") return null;
  return parseClipboardColor(record.content);
}

export function sourceLabelHtml(record: ClipboardRecord, query: string): string | undefined {
  const q = query.trim();
  if (!q) return undefined;
  return highlightSearchHtml(sourceShortName(record.source_app), q);
}

// Cached "now" refreshed at most once per 30s to avoid creating a Date object
// per row on every render (shared across all rows/components).
let cachedNow = Date.now();
let cachedNowTimer: ReturnType<typeof setTimeout> | null = null;

function getNow(): number {
  if (!cachedNowTimer) {
    cachedNowTimer = setTimeout(() => {
      cachedNow = Date.now();
      cachedNowTimer = null;
    }, 30_000);
  }
  return cachedNow;
}

export function formatTime(iso: string, t: TranslateFn): string {
  const d = new Date(iso);
  const diffMs = getNow() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return t('record.justNow');
  if (diffMin < 60) return t('record.minutesAgo', { n: diffMin });
  if (diffMin < 1440) return t('record.hoursAgo', { n: Math.floor(diffMin / 60) });
  return d.toLocaleDateString(undefined, { month: "numeric", day: "numeric" });
}
