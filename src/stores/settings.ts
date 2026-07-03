import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

const DEFAULT_SETTINGS: Settings = {
  global_shortcut: "Ctrl+Shift+V",
  max_records: 1000,
  retention_days: 30,
  theme: "dark",
  panel_opacity: 94,
  panel_radius: 20,
  enable_blur: true,
  enable_animation: true,
  font_size: 13,
  app_mode: "floating",
  default_paste_mode: "original",
  auto_close_on_paste: true,
  enable_sensitive_detection: true,
  sensitive_auto_expire_seconds: 600,
  data_path: "",
  auto_start: true,
  minimize_to_tray: true,
  ignored_apps: ["1Password.exe", "ICBCNetBank.exe"],
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>({ ...DEFAULT_SETTINGS });
  const isLoaded = ref(false);

  async function loadSettings() {
    try {
      const saved = await invoke<Settings>("get_settings");
      settings.value = { ...DEFAULT_SETTINGS, ...saved };
    } catch (e) {
      console.error("Failed to load settings:", e);
    } finally {
      isLoaded.value = true;
    }
    applyTheme(settings.value.theme);
    applyAppearance();
  }

  async function saveSettings() {
    try {
      await invoke("save_settings", { settings: settings.value });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  function applyTheme(theme: Settings["theme"]) {
    document.body.classList.remove("light-theme", "dark-theme", "oled-theme");
    if (theme === "system") {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.body.classList.add(prefersDark ? "dark-theme" : "light-theme");
    } else if (theme !== "dark") {
      document.body.classList.add(`${theme}-theme`);
    }
  }

  function applyAppearance() {
    const s = settings.value;
    const root = document.documentElement;

    // Font size
    root.style.fontSize = `${s.font_size}px`;

    // Panel radius (used as CSS variable)
    root.style.setProperty("--panel-radius", `${s.panel_radius}px`);

    // Panel opacity (used as CSS variable)
    root.style.setProperty("--panel-opacity", String(s.panel_opacity / 100));

    // Blur effect
    if (s.enable_blur) {
      document.body.classList.add("blur-enabled");
    } else {
      document.body.classList.remove("blur-enabled");
    }

    // Animations
    if (s.enable_animation) {
      document.body.classList.remove("anim-disabled");
    } else {
      document.body.classList.add("anim-disabled");
    }
  }

  function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings.value[key] = value;
    if (key === "theme") {
      applyTheme(value as Settings["theme"]);
    }
    // Apply appearance changes immediately for real-time preview
    if (["font_size", "panel_radius", "panel_opacity", "enable_blur", "enable_animation"].includes(key)) {
      applyAppearance();
    }
    // Auto-save is handled by the deep watch below
  }

  // Auto-save on changes (debounced)
  watch(
    settings,
    () => {
      if (isLoaded.value) {
        saveSettings();
      }
    },
    { deep: true }
  );

  return {
    settings,
    isLoaded,
    loadSettings,
    saveSettings,
    applyTheme,
    applyAppearance,
    updateSetting,
  };
});
