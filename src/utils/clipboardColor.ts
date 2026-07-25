/**
 * Detect when clipboard text is a standalone CSS color (not embedded in a doc).
 * Used for preview/list swatches — does not add a content_type.
 */

const HEX_RE = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

/** Classic comma rgb()/rgba() and hsl()/hsla(). */
const FUNC_RE =
  /^(rgba?|hsla?)\(\s*[\d.]+%?\s*,\s*[\d.]+%?\s*,\s*[\d.]+%?\s*(?:,\s*[\d.]+\s*)?\)$/i;

/** Modern space-separated: rgb(0 120 212) / rgb(0 120 212 / 50%). */
const FUNC_SPACE_RE =
  /^(rgba?|hsla?)\(\s*[\d.]+%?(?:\s+[\d.]+%?){2}(?:\s*\/\s*[\d.]+%?)?\s*\)$/i;

const MAX_LEN = 64;

/** Expand #abc → #aabbcc; #ab → invalid already excluded. */
export function expandHexColor(hex: string): string {
  const h = hex.trim();
  if (/^#[0-9a-fA-F]{3}$/.test(h)) {
    return `#${h[1]}${h[1]}${h[2]}${h[2]}${h[3]}${h[3]}`.toLowerCase();
  }
  if (/^#[0-9a-fA-F]{4}$/.test(h)) {
    return `#${h[1]}${h[1]}${h[2]}${h[2]}${h[3]}${h[3]}${h[4]}${h[4]}`.toLowerCase();
  }
  return h.toLowerCase();
}

/**
 * If `content` is only a color value, return a CSS color string for `background`.
 * Otherwise null.
 */
export function parseClipboardColor(content: string): string | null {
  const t = (content || "").trim();
  if (!t || t.length > MAX_LEN) return null;
  if (HEX_RE.test(t)) return expandHexColor(t);
  if (FUNC_RE.test(t) || FUNC_SPACE_RE.test(t)) return t;
  return null;
}
