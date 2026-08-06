/**
 * Tray-menu theme + DWM backdrop chrome, extracted from TrayMenuApp.vue so the
 * SFC script stays under 200 lines.
 */
import { invoke } from "@tauri-apps/api/core";

export interface TrayMenuState {
  paused: boolean;
  theme: string;
  enable_blur: boolean;
  blur_strength: number;
  enable_animation: boolean;
  panel_opacity: number;
  language: string;
}

/** Every theme class `applyTheme` can attach to <body> (mirrors settings store). */
const THEME_CLASSES = [
  "light-theme",
  "oled-theme",
  "dracula-theme",
  "nord-theme",
  "sunset-theme",
  "dracula-light-theme",
  "nord-light-theme",
  "sunset-light-theme",
] as const;

export function useTrayTheme() {
  function applyTheme(theme: string) {
    document.body.classList.remove(...THEME_CLASSES);
    if (theme !== "dark") {
      document.body.classList.add(`${theme}-theme`);
    }
  }

  function applyChrome(state: TrayMenuState) {
    applyTheme(state.theme);
    document.documentElement.style.setProperty(
      "--panel-opacity",
      String(state.panel_opacity / 100),
    );
    document.documentElement.style.setProperty(
      "--panel-blur-opacity",
      String((100 - state.blur_strength) / 100),
    );
    document.body.classList.toggle("blur-enabled", state.enable_blur);
    document.body.classList.toggle("anim-disabled", !state.enable_animation);
    // Apply native DWM acrylic to this window too (fresh window per tray right-click).
    invoke("set_window_backdrop", { enabled: state.enable_blur }).catch((e) => {
      console.error("set_window_backdrop failed:", e);
    });
  }

  return { applyChrome };
}
