/** Tag color picker is a 6×2 grid — always this many swatches. */
export const TAG_PALETTE_SIZE = 12;

/**
 * Fixed 12-color hue wheel (~30° steps). Must stay in sync with
 * `TAG_PALETTE` in `src-tauri/src/db/tags.rs`.
 */
export const TAG_PALETTE_HEX = [
  "#ef4444", // red
  "#f97316", // orange
  "#eab308", // amber
  "#84cc16", // lime
  "#22c55e", // green
  "#14b8a6", // teal
  "#06b6d4", // cyan
  "#0ea5e9", // sky
  "#3b82f6", // blue
  "#6366f1", // indigo
  "#a855f7", // purple
  "#ec4899", // pink
] as const;

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
  return uniqueColors([...extraColors, ...TAG_PALETTE_HEX]).slice(0, TAG_PALETTE_SIZE);
}

/** Named default-tag accents (aligned with seed / auto-tag colors). */
export function resolveKnownTagColors(): Record<string, string> {
  return {
    部署: "#22c55e",
    前端: "#6366f1",
    链接: "#eab308",
    重要: "#ef4444",
    设计: "#a855f7",
  };
}
