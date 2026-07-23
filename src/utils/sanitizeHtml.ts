import DOMPurify from "dompurify";

const CACHE_MAX = 24;
/** Fingerprint → sanitized HTML (avoids re-running DOMPurify on the same clipboard body). */
const sanitizeCache = new Map<string, string>();

function htmlCacheKey(html: string): string {
  // Cheap stable fingerprint — do not store the full HTML as the Map key.
  let h = html.length | 0;
  const step = Math.max(1, (html.length / 48) | 0);
  for (let i = 0; i < html.length; i += step) {
    h = (Math.imul(h, 31) + html.charCodeAt(i)) | 0;
  }
  return `${html.length}:${h}:${html.slice(0, 48)}:${html.slice(-48)}`;
}

/** Sanitize clipboard HTML for safe inline preview (no iframe). */
export function sanitizeClipboardHtml(html: string): string {
  const key = htmlCacheKey(html);
  const hit = sanitizeCache.get(key);
  if (hit !== undefined) return hit;

  const cleaned = DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    // Keep common rich-text bits from Word / browsers; strip scripts & handlers.
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "input", "link", "meta", "base"],
    FORBID_ATTR: ["srcset"],
    ALLOW_DATA_ATTR: false,
  });

  sanitizeCache.set(key, cleaned);
  if (sanitizeCache.size > CACHE_MAX) {
    const oldest = sanitizeCache.keys().next().value;
    if (oldest !== undefined) sanitizeCache.delete(oldest);
  }
  return cleaned;
}
