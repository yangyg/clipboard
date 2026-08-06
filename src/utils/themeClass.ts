/**
 * Theme class application shared by the settings store and the tray-menu window
 * chrome. Keeps the two `body`-level theme class lists in sync.
 */

import { THEME_DEFINITIONS } from "./themeRegistry";

/** Every non-default theme class `applyTheme` can attach to <body>. */
export const THEME_CLASSES = THEME_DEFINITIONS
  .filter(({ key }) => key !== "dark")
  .map(({ key }) => `${key}-theme`);

/** Attach the theme class for `theme` (dark = default, no class) to <body>. */
export function applyTheme(theme: string) {
  document.body.classList.remove(...THEME_CLASSES);
  if (theme !== "dark") {
    document.body.classList.add(`${theme}-theme`);
  }
}
