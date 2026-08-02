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

/** Resolve a CSS custom property to a concrete color string (usually hex). */
export function cssColorVar(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return raw || fallback;
}

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

function parseHexRgb(color: string): [number, number, number] | null {
  let h = color.trim().replace(/^#/, "");
  if (h.length === 3) {
    h = h
      .split("")
      .map((c) => c + c)
      .join("");
  }
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

/** Snap any hex to the nearest palette swatch (RGB Euclidean). Invalid → first swatch. */
export function nearestPaletteColor(color: string): string {
  const key = normalizeColorKey(color);
  const exact = TAG_PALETTE_HEX.find((c) => normalizeColorKey(c) === key);
  if (exact) return exact;

  const rgb = parseHexRgb(color);
  if (!rgb) return TAG_PALETTE_HEX[0];

  let best: string = TAG_PALETTE_HEX[0];
  let bestDist = Infinity;
  for (const swatch of TAG_PALETTE_HEX) {
    const s = parseHexRgb(swatch);
    if (!s) continue;
    const dr = rgb[0] - s[0];
    const dg = rgb[1] - s[1];
    const db = rgb[2] - s[2];
    const d = dr * dr + dg * dg + db * db;
    if (d < bestDist) {
      bestDist = d;
      best = swatch;
    }
  }
  return best;
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
