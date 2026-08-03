/**
 * Tray-menu theme + DWM backdrop chrome, extracted from TrayMenuApp.vue so the
 * SFC script stays under 200 lines. Owns the OS-theme cache that a hidden
 * WebView2 cannot get reliably from matchMedia.
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

export function useTrayTheme() {
  let currentTheme = "dark";
  // Latest authoritative OS dark-mode signal from the native watcher. matchMedia
  // can stay stale in this (mostly hidden) webview, so re-applying the "system"
  // theme on menu open must prefer the cache; null = no signal yet.
  let lastKnownSystemDark: boolean | null = null;

  function applyTheme(theme: string) {
    currentTheme = theme;
    document.body.classList.remove("light-theme", "dark-theme", "oled-theme");
    if (theme === "system") {
      const prefersDark =
        lastKnownSystemDark ?? window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.body.classList.add(prefersDark ? "dark-theme" : "light-theme");
    } else if (theme !== "dark") {
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

  /** Native OS light/dark change from Rust; only relevant while following the system theme. */
  function onSystemThemeChange(dark: boolean) {
    lastKnownSystemDark = dark;
    if (currentTheme !== "system") return;
    document.body.classList.remove("light-theme", "dark-theme", "oled-theme");
    document.body.classList.add(dark ? "dark-theme" : "light-theme");
  }

  return { applyChrome, onSystemThemeChange };
}
