/**
 * Theme class application shared by the settings store and the tray-menu window
 * chrome. Keeps the two `body`-level theme class lists in sync.
 */

/** Every theme class `applyTheme` can attach to <body>; removing all of them
 *  on re-apply keeps the tree clean when switching between any two themes. */
export const THEME_CLASSES = [
  "light-theme",
  "oled-theme",
  "dracula-theme",
  "nord-theme",
  "sunset-theme",
  "dracula-light-theme",
  "nord-light-theme",
  "sunset-light-theme",
  "handdrawn-theme",
  "handdrawn-light-theme",
] as const;

/** Attach the theme class for `theme` (dark = default, no class) to <body>. */
export function applyTheme(theme: string) {
  document.body.classList.remove(...THEME_CLASSES);
  if (theme !== "dark") {
    document.body.classList.add(`${theme}-theme`);
  }
}
