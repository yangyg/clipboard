/** Resolve a CSS custom property to a concrete color string (usually hex). */
export function cssColorVar(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return raw || fallback;
}

/**
 * Tag / auto-tag rule swatches resolved from theme tokens at call time.
 * Stored tag colors remain hex in SQLite; this only drives the picker defaults
 * and UI fallbacks so they track dark/light/OLED accent & type tokens.
 * Historical purple `#7c5cfc` is intentionally omitted.
 */
export const TAG_PALETTE_TOKEN_KEYS = [
  "--accent",
  "--accent-light",
  "--type-code",
  "--type-image",
  "--danger",
  "--sensitive",
  "--success",
  "--type-link",
  "--type-file",
  "--warning",
  "--text-secondary",
  "--text-tertiary",
] as const;

const TAG_PALETTE_FALLBACKS: Record<(typeof TAG_PALETTE_TOKEN_KEYS)[number], string> = {
  "--accent": "#6366f1",
  "--accent-light": "#818cf8",
  "--type-code": "#34d399",
  "--type-image": "#fbbf24",
  "--danger": "#f87171",
  "--sensitive": "#fb923c",
  "--success": "#34d399",
  "--type-link": "#6366f1",
  "--type-file": "#94a3b8",
  "--warning": "#fbbf24",
  "--text-secondary": "#8b8fa6",
  "--text-tertiary": "#868ba6",
};

export function resolveTagPalette(): string[] {
  return TAG_PALETTE_TOKEN_KEYS.map((key) => cssColorVar(key, TAG_PALETTE_FALLBACKS[key]));
}

/** Named default-tag accents (also token-backed). */
export function resolveKnownTagColors(): Record<string, string> {
  return {
    部署: cssColorVar("--type-code", "#34d399"),
    前端: cssColorVar("--accent", "#6366f1"),
    链接: cssColorVar("--type-image", "#fbbf24"),
    重要: cssColorVar("--danger", "#f87171"),
    设计: cssColorVar("--accent-light", "#818cf8"),
  };
}
