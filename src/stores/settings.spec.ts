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

describe("settingsStore system theme tracking", () => {
  const originalMatchMedia = window.matchMedia;

  /**
   * Installs a controllable prefers-color-scheme mock and returns helpers to
   * simulate the OS switching between light and dark mode.
   */
  function mockSystemTheme(initialDark: boolean) {
    type ChangeHandler = (e: { matches: boolean }) => void;
    const handlers = new Set<ChangeHandler>();
    const mql = {
      matches: initialDark,
      media: "(prefers-color-scheme: dark)",
      addEventListener: vi.fn((_event: string, handler: ChangeHandler) => {
        handlers.add(handler);
      }),
      removeEventListener: vi.fn((_event: string, handler: ChangeHandler) => {
        handlers.delete(handler);
      }),
    };
    window.matchMedia = vi.fn().mockReturnValue(mql as unknown as MediaQueryList);
    return {
      mql,
      handlers,
      /** Simulates the OS toggling night mode / dark mode. */
      setSystemDark(dark: boolean) {
        mql.matches = dark;
        for (const h of handlers) h({ matches: dark });
      },
    };
  }

  beforeEach(() => {
    setActivePinia(createPinia());
    document.body.className = "";
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
    // Keep the shared Tauri mocks pristine for the next test: drop recorded
    // calls (so handler lookups only see the current test's store) and any
    // unconsumed mockResolvedValueOnce, then restore the base implementation
    // installed in src/test/setup.ts.
    vi.mocked(listen).mockClear();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("applies dark-theme when the system prefers dark", () => {
    mockSystemTheme(true);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("dark-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);
  });

  it("applies light-theme when the system prefers light", () => {
    mockSystemTheme(false);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("light-theme")).toBe(true);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("follows live OS color-scheme changes while theme is system", () => {
    const { setSystemDark } = mockSystemTheme(false);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("light-theme")).toBe(true);

    setSystemDark(true);
    expect(document.body.classList.contains("dark-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);

    setSystemDark(false);
    expect(document.body.classList.contains("light-theme")).toBe(true);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("stops following OS changes after switching to a fixed theme", () => {
    const { mql, setSystemDark } = mockSystemTheme(false);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    store.updateSetting("theme", "light");
    expect(mql.removeEventListener).toHaveBeenCalledWith("change", expect.any(Function));

    setSystemDark(true);
    expect(document.body.classList.contains("light-theme")).toBe(true);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("does not register duplicate listeners when re-applying system theme", () => {
    const { mql, handlers } = mockSystemTheme(true);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    store.updateSetting("theme", "system");
    expect(mql.addEventListener).toHaveBeenCalledTimes(2);
    expect(handlers.size).toBe(1);
  });

  it("applies the saved system theme on loadSettings (app start)", async () => {
    mockSystemTheme(true);
    vi.mocked(invoke).mockResolvedValueOnce({ theme: "system" } as unknown as Settings);
    const store = useSettingsStore();
    await store.loadSettings();
    expect(store.settings.theme).toBe("system");
    expect(document.body.classList.contains("dark-theme")).toBe(true);
  });

  /**
   * Handler the store registered for the Rust-side "system-theme-changed"
   * event. Safe to scan the whole mock call list: afterEach clears it, so it
   * only ever contains the current test's registrations.
   */
  function lastNativeThemeHandler(): (e: { payload: boolean }) => void {
    const calls = vi.mocked(listen).mock.calls;
    for (let i = calls.length - 1; i >= 0; i--) {
      if (calls[i][0] === "system-theme-changed") {
        return calls[i][1] as unknown as (e: { payload: boolean }) => void;
      }
    }
    throw new Error("system-theme-changed listener was not registered");
  }

  it("applies the OS theme reported by the native watcher while theme is system", () => {
    mockSystemTheme(false);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("light-theme")).toBe(true);

    // Rust reports the OS flipped to dark (even though matchMedia, which is
    // stale in a hidden WebView2, still says light).
    lastNativeThemeHandler()({ payload: true });
    expect(document.body.classList.contains("dark-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);

    lastNativeThemeHandler()({ payload: false });
    expect(document.body.classList.contains("light-theme")).toBe(true);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("ignores native OS theme events when a fixed theme is active", () => {
    mockSystemTheme(true);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    store.updateSetting("theme", "oled");

    lastNativeThemeHandler()({ payload: false });
    expect(document.body.classList.contains("oled-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("ignores native OS theme events while the default dark theme is active", () => {
    mockSystemTheme(true);
    const store = useSettingsStore();
    expect(store.settings.theme).toBe("dark");

    lastNativeThemeHandler()({ payload: false });
    // Default dark theme renders via base CSS — no theme class is added.
    expect(document.body.classList.contains("light-theme")).toBe(false);
    expect(document.body.classList.contains("dark-theme")).toBe(false);
  });

  it("keeps the native-reported theme across a runtime loadSettings (stale matchMedia)", async () => {
    const { mql } = mockSystemTheme(false); // matchMedia stuck on light (stale)
    const store = useSettingsStore();
    store.updateSetting("theme", "system");

    lastNativeThemeHandler()({ payload: true }); // authoritative: dark
    expect(document.body.classList.contains("dark-theme")).toBe(true);

    // A runtime loadSettings (e.g. after a WebDAV sync) must not revert to
    // the stale matchMedia value.
    vi.mocked(invoke).mockResolvedValueOnce({ theme: "system" } as unknown as Settings);
    await store.loadSettings();
    expect(mql.matches).toBe(false); // still stale — proves the cache was used
    expect(document.body.classList.contains("dark-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);
  });

  it("re-applies the cached system theme when switching back from a fixed theme", () => {
    mockSystemTheme(false); // stale light
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    lastNativeThemeHandler()({ payload: true }); // authoritative: dark

    store.updateSetting("theme", "light");
    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("dark-theme")).toBe(true);
    expect(document.body.classList.contains("light-theme")).toBe(false);
  });
});

describe("settingsStore colorful preset themes", () => {
  const originalMatchMedia = window.matchMedia;

  function mockSystemTheme(initialDark: boolean) {
    window.matchMedia = vi.fn().mockReturnValue({
      matches: initialDark,
      media: "(prefers-color-scheme: dark)",
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    } as unknown as MediaQueryList);
  }

  beforeEach(() => {
    setActivePinia(createPinia());
    document.body.className = "";
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
    vi.mocked(listen).mockClear();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
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

  it("removes light colorful classes when switching to system", () => {
    mockSystemTheme(false);
    const store = useSettingsStore();
    store.updateSetting("theme", "sunset-light");
    expect(document.body.classList.contains("sunset-light-theme")).toBe(true);

    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("sunset-light-theme")).toBe(false);
    expect(document.body.classList.contains("light-theme")).toBe(true);
  });

  it("removes colorful classes when switching to system", () => {
    mockSystemTheme(true);
    const store = useSettingsStore();
    store.updateSetting("theme", "sunset");
    expect(document.body.classList.contains("sunset-theme")).toBe(true);

    store.updateSetting("theme", "system");
    expect(document.body.classList.contains("sunset-theme")).toBe(false);
    expect(document.body.classList.contains("dark-theme")).toBe(true);
  });

  it("ignores native OS theme events while a colorful fixed theme is active", () => {
    mockSystemTheme(true);
    const store = useSettingsStore();
    store.updateSetting("theme", "system");
    store.updateSetting("theme", "dracula");

    const calls = vi.mocked(listen).mock.calls;
    const handler = calls.find((c) => c[0] === "system-theme-changed")?.[1] as
      | ((e: { payload: boolean }) => void)
      | undefined;
    handler?.({ payload: false });
    expect(document.body.classList.contains("dracula-theme")).toBe(true);
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
});
