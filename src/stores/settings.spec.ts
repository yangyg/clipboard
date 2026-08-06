import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettingsStore } from "@/stores/settings";
import type { Settings } from "@/types";

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
    expect(store.settings.font_family).toBe("default");
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


describe("settingsStore colorful preset themes", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    document.body.className = "";
  });

  afterEach(() => {
    // Clear call history + once-implementations only. mockReset() would wipe
    // the setup.ts default (get_records → empty page), silently breaking any
    // later list-returning store action.
    vi.mocked(listen).mockClear();
    vi.mocked(invoke).mockClear();
  });

  it("applies the colorful theme class to <body>", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "dracula");
    expect(document.body.classList.contains("dracula-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("applies each colorful theme with its own class", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "nord");
    expect(document.body.classList.contains("nord-theme")).toBe(true);
    store.updateSetting("theme", "sunset");
    expect(document.body.classList.contains("sunset-theme")).toBe(true);
    expect(document.body.classList.contains("nord-theme")).toBe(false);
    expect(document.body.classList.contains("dracula-theme")).toBe(false);
  });

  it("applies light colorful themes with their own classes", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "dracula-light");
    expect(document.body.classList.contains("dracula-light-theme")).toBe(true);
    expect(document.body.classList.contains("dracula-theme")).toBe(false);

    store.updateSetting("theme", "nord-light");
    expect(document.body.classList.contains("nord-light-theme")).toBe(true);
    expect(document.body.classList.contains("dracula-light-theme")).toBe(false);

    store.updateSetting("theme", "sunset-light");
    expect(document.body.classList.contains("sunset-light-theme")).toBe(true);
    expect(document.body.classList.contains("nord-light-theme")).toBe(false);
  });

  it("removes colorful classes when switching to a base theme", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "sunset");
    expect(document.body.classList.contains("sunset-theme")).toBe(true);

    store.updateSetting("theme", "light");
    expect(document.body.classList.contains("sunset-theme")).toBe(false);
    expect(document.body.classList.contains("light-theme")).toBe(true);
  });

  it("removes light colorful classes when switching to a base theme", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "sunset-light");
    expect(document.body.classList.contains("sunset-light-theme")).toBe(true);

    store.updateSetting("theme", "dark");
    expect(document.body.classList.contains("sunset-light-theme")).toBe(false);
    expect(document.body.classList.contains("light-theme")).toBe(false);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("applies a saved colorful theme on loadSettings (app start)", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ theme: "nord" } as unknown as Settings);
    const store = useSettingsStore();
    await store.loadSettings();
    expect(store.settings.theme).toBe("nord");
    expect(document.body.classList.contains("nord-theme")).toBe(true);
  });

  it("applies handdrawn themes with their own classes", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "handdrawn");
    expect(document.body.classList.contains("handdrawn-theme")).toBe(true);
    expect(document.body.classList.contains("handdrawn-light-theme")).toBe(false);

    store.updateSetting("theme", "handdrawn-light");
    expect(document.body.classList.contains("handdrawn-light-theme")).toBe(true);
    expect(document.body.classList.contains("handdrawn-theme")).toBe(false);
  });

  it("removes handdrawn classes when switching away", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "handdrawn");
    expect(document.body.classList.contains("handdrawn-theme")).toBe(true);

    store.updateSetting("theme", "dark");
    expect(document.body.classList.contains("handdrawn-theme")).toBe(false);
    expect(document.body.classList.contains("handdrawn-light-theme")).toBe(false);
  });

  it("applies mono themes with their own classes", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "mono");
    expect(document.body.classList.contains("mono-theme")).toBe(true);
    expect(document.body.classList.contains("mono-light-theme")).toBe(false);

    store.updateSetting("theme", "mono-light");
    expect(document.body.classList.contains("mono-light-theme")).toBe(true);
    expect(document.body.classList.contains("mono-theme")).toBe(false);
  });

  it("removes mono classes when switching away", () => {
    const store = useSettingsStore();
    store.updateSetting("theme", "mono");
    expect(document.body.classList.contains("mono-theme")).toBe(true);

    store.updateSetting("theme", "dark");
    expect(document.body.classList.contains("mono-theme")).toBe(false);
    expect(document.body.classList.contains("mono-light-theme")).toBe(false);
  });

  it("normalizes a legacy 'system' theme value to dark on loadSettings", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ theme: "system" } as unknown as Settings);
    const store = useSettingsStore();
    await store.loadSettings();
    expect(store.settings.theme).toBe("dark");
  });
});
