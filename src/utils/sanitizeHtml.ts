import DOMPurify from "dompurify";

/** Sanitize clipboard HTML for safe inline preview (no iframe). */
export function sanitizeClipboardHtml(html: string): string {
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    // Keep common rich-text bits from Word / browsers; strip scripts & handlers.
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "input", "link", "meta", "base"],
    FORBID_ATTR: ["srcset"],
    ALLOW_DATA_ATTR: false,
  });
}
