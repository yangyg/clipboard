import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";
import { DEFAULT_AUTO_TAG_RULES } from "../types";
import { DEFAULT_FEATURES, mergeFeatures } from "../features/capabilities";
import { useToast } from "../composables/useToast";
import { resolveFontStack } from "../utils/fontPresets";
import { applyTheme as applyThemeClass } from "../utils/themeClass";
import { isThemeKey } from "../utils/themeRegistry";
import { i18n } from "../locales";

const DEFAULT_SETTINGS: Settings = {
  global_shortcut: "Ctrl+Shift+V",
  max_records: 1000,
  retention_days: 30,
  theme: "dark",
  panel_opacity: 94,
  panel_radius: 20,
  enable_blur: false,
  blur_strength: 45,
  enable_animation: true,
  font_size: 16,
  font_family: "default",
  search_mode: "full",
  always_on_top: false,
  default_paste_mode: "original",
  auto_close_on_paste: true,
  enable_sensitive_detection: true,
  sensitive_auto_expire_seconds: 600,
  max_text_bytes: 10 * 1024 * 1024,
  import_system_history_on_start: false,
  auto_start: false,
  minimize_to_tray: true,
  ignored_apps: [
    "1Password.exe",
    "Bitwarden.exe",
    "KeePass.exe",
    "KeePassXC.exe",
    "Enpass.exe",
    "Dashlane.exe",
    "ICBCNetBank.exe",
  ],
  source_name_overrides: [],
  window_width: 0,
  window_height: 0,
  enable_auto_tag: true,
  auto_tag_rules: DEFAULT_AUTO_TAG_RULES.map((r) => ({
    ...r,
    keywords: [...r.keywords],
    content_types: [...r.content_types],
  })),
  onboarding_completed: false,
  language: 'system',
  webdav_url: "",
  webdav_username: "",
  webdav_password: "",
  webdav_remote_path: "ClipVaultSync",
  webdav_sync_sensitive: false,
  webdav_device_id: "",
  webdav_device_name: "",
  webdav_device_names: {},
  webdav_last_sync_at: null,
  enable_ai: false,
  ai_base_url: "https://api.openai.com/v1",
  ai_api_key: "",
  ai_model: "gpt-4o-mini",
  ai_summary_alias: true,
  ai_auto_tag: true,
  ai_max_chars: 4000,
  ai_min_chars: 32,
  features: { ...DEFAULT_FEATURES },
};

function normalizeSettings(raw: Partial<Settings> | Settings | undefined): Settings {
  const rawTheme = raw?.theme as string | undefined;
  return {
    ...DEFAULT_SETTINGS,
    ...(raw ?? {}),
    features: mergeFeatures(raw?.features),
    // Legacy "system" theme value (removed): collapse to the dark default so
    // existing users don't end up with an unselected theme card.
    theme:
      rawTheme === "system"
        ? "dark"
        : rawTheme && isThemeKey(rawTheme)
          ? rawTheme
          : DEFAULT_SETTINGS.theme,
    default_paste_mode: raw?.default_paste_mode === "plain" ? "plain" : "original",
    search_mode:
      raw?.search_mode === "icon" || raw?.search_mode === "hidden"
        ? raw.search_mode
        : "full",
    auto_tag_rules: (raw?.auto_tag_rules ?? DEFAULT_SETTINGS.auto_tag_rules).map((r) => ({
      ...r,
      keywords: [...r.keywords],
      content_types: [...r.content_types],
    })),
    language:
      raw?.language === "en-US" || raw?.language === "zh-CN" || raw?.language === "system"
        ? raw.language
        : DEFAULT_SETTINGS.language,
  };
}

const SAVE_DEBOUNCE_MS = 200;

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>(normalizeSettings(DEFAULT_SETTINGS));
  const isLoaded = ref(false);
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveGeneration = 0;
  /** Set when restoring settings after a failed save. The restore assignment
   * fires the deep watch only AFTER isLoaded is back to true (watch jobs flush
   * later), so isLoaded alone cannot suppress the redundant retry — an explicit
   * one-shot flag can. */
  let suppressNextSave = false;

  async function loadSettings() {
    try {
      const saved = await invoke<Settings>("get_settings");
      settings.value = normalizeSettings(saved);
    } catch (e) {
      console.error("Failed to load settings:", e);
    } finally {
      isLoaded.value = true;
    }
    applyTheme(settings.value.theme);
    applyAppearance();
  }

  async function saveSettings() {
    const generation = ++saveGeneration;
    const snapshot = normalizeSettings(settings.value);
    try {
      await invoke("save_settings", { settings: snapshot });
    } catch (e) {
      console.error("Failed to save settings:", e);
      if (generation !== saveGeneration) return;
      // Reload so UI matches DB after failed OS sync / save (suppress auto-save while restoring)
      isLoaded.value = false;
      try {
        const saved = await invoke<Settings>("get_settings");
        suppressNextSave = true; // the restore below must not re-trigger auto-save
        settings.value = normalizeSettings(saved);
        applyTheme(settings.value.theme);
        applyAppearance();
      } catch (reloadErr) {
        console.error("Failed to reload settings after save error:", reloadErr);
      } finally {
        isLoaded.value = true;
      }
      useToast().toast(i18n.global.t("settings.saveFailed"), "error");
    }
  }

  function scheduleSave() {
    if (!isLoaded.value) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void saveSettings();
    }, SAVE_DEBOUNCE_MS);
  }

  function applyTheme(theme: Settings["theme"]) {
    applyThemeClass(theme);
  }

  const lastAppliedRadius = ref<number | null>(null);
  const lastAppliedBlur = ref<boolean | null>(null);

  function applyAppearance() {
    const s = settings.value;
    const root = document.documentElement;

    // Font scale relative to 16px rem baseline (components use rem)
    root.style.setProperty("--ui-font-scale", String(s.font_size / 16));
    root.style.fontSize = `${s.font_size}px`;

    // UI font family (preset key or `system:<name>`)
    root.style.setProperty("--font-sans", resolveFontStack(s.font_family));

    // Panel radius (used as CSS variable)
    root.style.setProperty("--panel-radius", `${s.panel_radius}px`);

    // Clip HWND only when radius actually changes (avoid IPC on every font/opacity tick)
    if (lastAppliedRadius.value !== s.panel_radius) {
      lastAppliedRadius.value = s.panel_radius;
      void invoke("set_window_corner_radius", { radius: s.panel_radius }).catch((e) => {
        console.error("Failed to set window corner radius:", e);
      });
    }

    // Native frosted-glass backdrop (DWM acrylic) — CSS backdrop-filter can't
    // blur the OS desktop behind a transparent WebView2 window.
    if (lastAppliedBlur.value !== s.enable_blur) {
      lastAppliedBlur.value = s.enable_blur;
      void invoke("set_window_backdrop", { enabled: s.enable_blur }).catch((e) => {
        console.error("Failed to set window backdrop:", e);
      });
    }

    // Panel opacity (used as CSS variable)
    root.style.setProperty("--panel-opacity", String(s.panel_opacity / 100));

    // Frosted-glass surface tint opacity (100 - strength): higher strength →
    // more blurred desktop shows through when 毛玻璃 is enabled.
    root.style.setProperty("--panel-blur-opacity", String((100 - s.blur_strength) / 100));

    // Blur: applies to .panel-surface chrome
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
    if (
      ["font_size", "font_family", "panel_radius", "panel_opacity", "enable_blur", "blur_strength", "enable_animation"].includes(
        key as string,
      )
    ) {
      applyAppearance();
    }
    // Auto-save is handled by the deep watch below
  }

  // Auto-save on changes (debounced)
  watch(
    settings,
    () => {
      if (suppressNextSave) {
        suppressNextSave = false;
        return;
      }
      scheduleSave();
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
