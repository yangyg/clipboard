/**
 * UI font-family presets + resolution for the `font_family` setting.
 *
 * `font_family` is either a preset key (default/yahei/simhei/simsun/kaiti/segoe)
 * or a system font chosen from the OS list, stored as `system:<family name>`.
 * Every stack carries a CJK-capable fallback (Microsoft YaHei UI is the
 * Windows Chinese default) so mixed Latin/CJK text never falls back to the
 * browser default.
 */

/** Stack applied to --font-sans when a system-font choice lacks a preset. */
export const SYSTEM_FONT_FALLBACK =
  '"Microsoft YaHei UI", "Microsoft YaHei", sans-serif';

export interface FontPreset {
  key: string;
  labelKey: string;
  stack: string;
}

export const FONT_PRESETS: readonly FontPreset[] = [
  {
    key: "default",
    labelKey: "settings.appearance.fontDefault",
    stack: '"Noto Sans SC", -apple-system, BlinkMacSystemFont, sans-serif',
  },
  {
    key: "yahei",
    labelKey: "settings.appearance.fontYahei",
    stack: '"Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans SC", sans-serif',
  },
  {
    key: "simhei",
    labelKey: "settings.appearance.fontSimHei",
    stack: '"SimHei", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
  },
  {
    key: "simsun",
    labelKey: "settings.appearance.fontSimSun",
    stack: '"SimSun", "Microsoft YaHei UI", serif',
  },
  {
    key: "kaiti",
    labelKey: "settings.appearance.fontKaiTi",
    stack: '"KaiTi", "STKaiti", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
  },
  {
    key: "segoe",
    labelKey: "settings.appearance.fontSegoe",
    stack: '"Segoe UI", "Microsoft YaHei UI", "Noto Sans SC", sans-serif',
  },
];

const PRESET_BY_KEY = new Map(FONT_PRESETS.map((p) => [p.key, p.stack]));

const SYSTEM_PREFIX = "system:";

/** True when `font_family` points at an OS-installed font (not a preset). */
export function isSystemFontValue(fontFamily: string): boolean {
  return fontFamily.startsWith(SYSTEM_PREFIX);
}

/** Family name of a system-font choice ("" for non-system values). */
export function systemFontName(fontFamily: string): string {
  return isSystemFontValue(fontFamily) ? fontFamily.slice(SYSTEM_PREFIX.length) : "";
}

/**
 * Resolve `font_family` to a CSS font stack for --font-sans.
 * Preset key → preset stack; `system:<name>` → the family with a CJK-safe
 * fallback; anything unknown → the default preset stack.
 */
export function resolveFontStack(fontFamily: string): string {
  if (isSystemFontValue(fontFamily)) {
    const name = systemFontName(fontFamily);
    if (name) return `${JSON.stringify(name)}, ${SYSTEM_FONT_FALLBACK}`;
  }
  return PRESET_BY_KEY.get(fontFamily) ?? PRESET_BY_KEY.get("default")!;
}

/** All option values (presets + the system-font marker) for the settings select. */
export const FONT_OPTIONS = [
  ...FONT_PRESETS.map((p) => p.key),
  SYSTEM_PREFIX,
] as const;

export const SYSTEM_FONT_OPTION_KEY = SYSTEM_PREFIX;
