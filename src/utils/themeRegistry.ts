export const THEME_DEFINITIONS = [
  { key: "dark", icon: "moon", labelKey: "settings.appearance.themeDark" },
  { key: "light", icon: "sun", labelKey: "settings.appearance.themeLight" },
  { key: "oled", icon: "circle", labelKey: "settings.appearance.themeOled" },
  { key: "dracula", icon: "sparkles", labelKey: "settings.appearance.themeDracula" },
  { key: "dracula-light", icon: "sparkles", labelKey: "settings.appearance.themeDraculaLight" },
  { key: "nord", icon: "zap", labelKey: "settings.appearance.themeNord" },
  { key: "nord-light", icon: "zap", labelKey: "settings.appearance.themeNordLight" },
  { key: "sunset", icon: "star", labelKey: "settings.appearance.themeSunset" },
  { key: "sunset-light", icon: "star", labelKey: "settings.appearance.themeSunsetLight" },
  { key: "handdrawn", icon: "edit", labelKey: "settings.appearance.themeHanddrawn" },
  { key: "handdrawn-light", icon: "palette", labelKey: "settings.appearance.themeHanddrawnLight" },
  { key: "mono", icon: "circle", labelKey: "settings.appearance.themeMono" },
  { key: "mono-light", icon: "circle", labelKey: "settings.appearance.themeMonoLight" },
  { key: "editorial", icon: "component", labelKey: "settings.appearance.themeEditorial" },
  { key: "editorial-light", icon: "component", labelKey: "settings.appearance.themeEditorialLight" },
  { key: "sticker", icon: "palette", labelKey: "settings.appearance.themeSticker" },
  { key: "sticker-light", icon: "palette", labelKey: "settings.appearance.themeStickerLight" },
] as const;

export type ThemeKey = (typeof THEME_DEFINITIONS)[number]["key"];

export function isThemeKey(value: string): value is ThemeKey {
  return THEME_DEFINITIONS.some((theme) => theme.key === value);
}
