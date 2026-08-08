import DOMPurify from "dompurify";

const CACHE_MAX = 24;
/** HTML → sanitized HTML (avoids re-running DOMPurify on the same clipboard body). */
const sanitizeCache = new Map<string, string>();

/** Sanitize clipboard HTML for safe inline preview (no iframe). */
export function sanitizeClipboardHtml(html: string): string {
  // The full HTML is the cache key. A sampled fingerprint (length + every Nth
  // char + head/tail) can collide across different bodies and would return
  // another record's sanitized output; the cache is bounded to CACHE_MAX
  // entries so keying on the whole string costs at most a few KB per entry.
  const hit = sanitizeCache.get(html);
  if (hit !== undefined) return hit;

  const cleaned = DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    // Keep common rich-text bits from Word / browsers; strip scripts & handlers.
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "input", "link", "meta", "base"],
    FORBID_ATTR: ["srcset"],
    ALLOW_DATA_ATTR: false,
    // Block javascript:/data:/vbscript: in href/src
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):)/i,
  });

  sanitizeCache.set(html, cleaned);
  if (sanitizeCache.size > CACHE_MAX) {
    const oldest = sanitizeCache.keys().next().value;
    if (oldest !== undefined) sanitizeCache.delete(oldest);
  }
  return cleaned;
}
