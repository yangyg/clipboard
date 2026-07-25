/** Resolve a CSS custom property to a concrete color string (usually hex). */
export function cssColorVar(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return raw || fallback;
}

/** Tag color picker is a 6×2 grid — always this many swatches. */
export const TAG_PALETTE_SIZE = 12;

/**
 * Tag / auto-tag rule swatches (12 slots).
 * Stored tag colors remain hex in SQLite; picker resolves theme tokens at call time.
 * Dropped unused `--type-text` (aliased `--text-secondary`).
 * When a live token collides with an earlier slot, that slot's distinct fallback is used.
 */
export const TAG_PALETTE_TOKEN_KEYS = [
  "--accent",
  "--accent-light",
  "--accent-hover",
  "--type-code",
  "--success",
  "--type-image",
  "--warning",
  "--danger",
  "--sensitive",
  "--type-link",
  "--type-file",
  "--text-secondary",
] as const;

/** Distinct fallbacks — one per slot so the picker stays 12 unique colors. */
const TAG_PALETTE_FALLBACKS: Record<(typeof TAG_PALETTE_TOKEN_KEYS)[number], string> = {
  "--accent": "#0078d4",
  "--accent-light": "#60cdff",
  "--accent-hover": "#1b86d9",
  "--type-code": "#34d399",
  "--success": "#2dd4bf",
  "--type-image": "#0ea5e9",
  "--warning": "#f59e0b",
  "--danger": "#f87171",
  "--sensitive": "#fb923c",
  "--type-link": "#2563eb",
  "--type-file": "#eab308",
  "--text-secondary": "#8b8fa6",
};

/** Normalize for equality (hex case / whitespace). */
export function normalizeColorKey(color: string): string {
  return color.trim().toLowerCase();
}

/** Keep first occurrence of each distinct color. */
export function uniqueColors(colors: string[]): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const c of colors) {
    const key = normalizeColorKey(c);
    if (!key || seen.has(key)) continue;
    seen.add(key);
    out.push(c.trim());
  }
  return out;
}

/**
 * Build exactly {@link TAG_PALETTE_SIZE} swatches.
 * `extraColors` (editing / existing tags) take priority, still capped at 12.
 */
export function resolveTagPalette(extraColors: string[] = []): string[] {
  const seen = new Set<string>();
  const fromTheme: string[] = [];
  for (const key of TAG_PALETTE_TOKEN_KEYS) {
    const fallback = TAG_PALETTE_FALLBACKS[key];
    const live = cssColorVar(key, fallback).trim();
    const liveKey = normalizeColorKey(live);
    const pick = liveKey && !seen.has(liveKey) ? live : fallback;
    const pickKey = normalizeColorKey(pick);
    if (!pickKey || seen.has(pickKey)) continue;
    seen.add(pickKey);
    fromTheme.push(pick.trim());
  }
  return uniqueColors([...extraColors, ...fromTheme]).slice(0, TAG_PALETTE_SIZE);
}

/** Named default-tag accents (also token-backed). */
export function resolveKnownTagColors(): Record<string, string> {
  return {
    部署: cssColorVar("--type-code", "#34d399"),
    前端: cssColorVar("--accent", "#0078d4"),
    链接: cssColorVar("--type-link", "#2563eb"),
    重要: cssColorVar("--danger", "#f87171"),
    设计: cssColorVar("--accent-light", "#60cdff"),
  };
}
