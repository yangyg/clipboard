import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useSettingsStore } from "@/stores/settings";

describe("settingsStore (smoke)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    document.body.className = "";
  });

  it("initializes with sensible defaults", () => {
    const store = useSettingsStore();
    expect(store.settings.theme).toBe("dark");
    expect(store.settings.app_mode).toBe("floating");
    expect(store.settings.font_size).toBe(16);
    expect(store.settings.max_records).toBe(1000);
    expect(store.settings.onboarding_completed).toBe(false);
    expect(store.isLoaded).toBe(false);
  });

  it("updates a plain setting value in place", () => {
    const store = useSettingsStore();
    store.updateSetting("max_records", 500);
    expect(store.settings.max_records).toBe(500);
  });

  it("applies the theme class to <body> when theme changes", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "light");
    expect(document.body.classList.contains("light-theme")).toBe(true);
    store.updateSetting("theme", "dark");
    expect(document.body.classList.contains("light-theme")).toBe(false);
  });
});
