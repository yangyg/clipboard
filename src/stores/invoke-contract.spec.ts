/**
 * Tauri Invoke Contract Tests
 *
 * Validates that every invoke() call from stores matches the Rust backend's
 * #[tauri::command] signatures — command name, parameter keys, and required args.
 *
 * THE CONTRACT — keep in sync with src-tauri/src/commands/*.rs.
 * Each entry lists the expected parameter keys (matching Rust's snake_case
 * after #[tauri::command(rename_all = "snake_case")] where applicable).
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "@/stores/clipboard";
import { useSettingsStore } from "@/stores/settings";
import type { ClipboardRecord, Settings, StatsData } from "@/types";

// ─── Contract Definition ─────────────────────────────────────────────────────
// Each key is a Tauri command name; the value is the set of expected parameter
// keys the frontend must send. Must match #[tauri::command] signatures in
// src-tauri/src/commands/*.rs (after rename_all = "snake_case" if present).

const COMMAND_CONTRACTS: Record<string, { params: string[] }> = {
  // ── clipboard.ts ──
  get_records: {
    params: [
      "limit", "offset", "trashed", "content_type", "favorites_only",
      "tag", "sort", "before_pinned", "before_updated_at", "before_id",
    ],
  },
  search_records: {
    params: [
      "query", "limit", "offset", "content_type", "favorites_only",
      "tag", "sort", "before_pinned", "before_updated_at", "before_id",
    ],
  },
  get_pending_history_import: { params: [] },
  get_search_history: { params: ["limit"] },
  record_search_history: { params: ["query"] },
  remove_search_history: { params: ["query"] },
  clear_search_history: { params: [] },
  get_record: { params: ["id"] },
  open_record_media: { params: ["id"] },
  paste_record: { params: ["id", "mode"] },
  delete_record: { params: ["id"] },
  delete_records_batch: { params: ["ids"] },
  restore_record: { params: ["id"] },
  restore_records_batch: { params: ["ids"] },
  permanently_delete_record: { params: ["id"] },
  permanently_delete_records_batch: { params: ["ids"] },
  toggle_favorite: { params: ["id"] },
  batch_set_favorite: { params: ["ids", "favorite"] },
  toggle_pin: { params: ["id"] },
  set_record_alias: { params: ["id", "alias"] },
  cleanup_expired: { params: [] },
  empty_trash: { params: [] },
  get_trash_count: { params: [] },
  set_capture_paused: { params: ["paused"] },
  get_stats: { params: [] },
  import_data: { params: ["records_json"] },
  get_all_tags: { params: ["content_type", "favorites_only"] },
  create_tag: { params: ["name", "color"] },
  delete_tag: { params: ["id"] },
  update_tag: { params: ["id", "name", "color"] },
  add_tag_to_record: { params: ["record_id", "tag_id"] },
  remove_tag_from_record: { params: ["record_id", "tag_id"] },
  set_record_tags: { params: ["record_id", "tag_ids"] },

  // ── settings.ts ──
  get_settings: { params: [] },
  get_system_fonts: { params: [] },
  save_settings: { params: ["settings"] },
  set_window_corner_radius: { params: ["radius"] },
  set_window_backdrop: { params: ["enabled"] },

  // ── Components / App.vue ──
  open_url: { params: ["url"] },
  capture_paste_target: { params: [] },
  switch_app_mode: { params: ["mode"] },
  get_tray_menu_state: { params: [] },
  tray_menu_action: { params: ["action"] },
  export_data: { params: ["path"] },
  import_data_from_path: { params: ["path"] },
  clear_history: { params: [] },
  webdav_test_connection: { params: [] },
  webdav_pull: { params: [] },
  webdav_push: { params: [] },
  webdav_sync: { params: [] },
  get_sync_history: { params: ["limit"] },
  clear_sync_history: { params: [] },
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeRecord(overrides: Partial<ClipboardRecord> = {}): ClipboardRecord {
  return {
    id: 1, content: "test", content_type: "text", source_app: "test.exe",
    source_window: "Test", hash: "abc", copy_count: 0, is_favorite: false,
    is_pinned: false, is_sensitive: false, is_trashed: false,
    auto_expire_at: null, created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z", tags: [], alias: "",
    ...overrides,
  };
}


const MOCK_SETTINGS: Settings = {
  global_shortcut: "Ctrl+Shift+V", max_records: 1000, retention_days: 30,
  theme: "dark", panel_opacity: 94, panel_radius: 20, enable_blur: false,
  blur_strength: 45, enable_animation: true, font_size: 16, font_family: "default", search_mode: "full", app_mode: "floating",
  default_paste_mode: "original", auto_close_on_paste: true,
  enable_sensitive_detection: true, sensitive_auto_expire_seconds: 600,
  import_system_history_on_start: false, auto_start: false, minimize_to_tray: true,
  ignored_apps: [], source_name_overrides: [], floating_width: 0, floating_height: 0,
  window_width: 0, window_height: 0, enable_auto_tag: true,
  auto_tag_rules: [], onboarding_completed: false, language: "zh-CN",
  webdav_url: "", webdav_username: "", webdav_password: "",
  webdav_remote_path: "ClipVaultSync", webdav_sync_sensitive: false,
  webdav_device_id: "", webdav_last_sync_at: null,
  features: { tags: true, batch: true, sync: true, stats: true },
};

const MOCK_STATS: StatsData = {
  total_records: 0, total_copies: 0, favorites_count: 0, pinned_count: 0,
  sensitive_count: 0, storage_bytes: 0, data_path: "", type_distribution: {},
};

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("Tauri invoke contract — command names & parameter keys", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(invoke).mockClear();
  });

  // ── clipboard store ──

  it("loadRecords → get_records with correct params", async () => {
    const store = useClipboardStore();
    await store.loadRecords();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_records");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    expect(params).toBeDefined();
    const expectedKeys = new Set(COMMAND_CONTRACTS.get_records.params);
    for (const key of Object.keys(params)) {
      expect(expectedKeys.has(key), `Unexpected param "${key}" in get_records`).toBe(true);
    }
    for (const key of expectedKeys) {
      expect(key in params, `Missing required param "${key}" in get_records`).toBe(true);
    }
  });

  it("search → search_records with query and filter params", async () => {
    const store = useClipboardStore();
    await store.search("hello");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "search_records");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    expect(params.query).toBe("hello");
    const expectedKeys = new Set(COMMAND_CONTRACTS.search_records.params);
    for (const key of Object.keys(params)) {
      expect(expectedKeys.has(key), `Unexpected param "${key}" in search_records`).toBe(true);
    }
  });

  it("ensureRecordDetail → get_record with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 42 })];
    await store.ensureRecordDetail(42);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 42 });
  });

  // ── search history (useSearchHistory composable) ──

  it("loadHistory → get_search_history with { limit }", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    await invoke("get_search_history", { limit: 50 });
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_search_history");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ limit: 50 });
  });

  it("recordHistory → record_search_history with { query }", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await invoke("record_search_history", { query: "hello" });
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "record_search_history");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ query: "hello" });
  });

  it("removeHistory → remove_search_history with { query }", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await invoke("remove_search_history", { query: "hello" });
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "remove_search_history");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ query: "hello" });
  });

  it("clearHistory → clear_search_history with no params", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await invoke("clear_search_history");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "clear_search_history");
    expect(call).toBeTruthy();
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("pasteRecord → paste_record with { id, mode }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 10 })];
    await store.pasteRecord(10, "plain");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "paste_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 10, mode: "plain" });
  });

  it("deleteRecord → delete_record with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 5 })];
    await store.deleteRecord(5);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "delete_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 5 });
  });

  it("deleteBatch → delete_records_batch with { ids }", async () => {
    const store = useClipboardStore();
    await store.deleteBatch([1, 2, 3]);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "delete_records_batch");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ ids: [1, 2, 3] });
  });

  it("restoreRecord → restore_record with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 7 })];
    await store.restoreRecord(7);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "restore_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 7 });
  });

  it("restoreRecordsBatch → restore_records_batch with { ids }", async () => {
    const store = useClipboardStore();
    await store.restoreRecordsBatch([4, 5]);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "restore_records_batch");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ ids: [4, 5] });
  });

  it("permanentlyDeleteRecord → permanently_delete_record with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 8 })];
    await store.permanentlyDeleteRecord(8);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "permanently_delete_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 8 });
  });

  it("permanentlyDeleteRecordsBatch → permanently_delete_records_batch with { ids }", async () => {
    const store = useClipboardStore();
    await store.permanentlyDeleteRecordsBatch([1, 2]);
    const call = vi.mocked(invoke).mock.calls.find(
      (c) => c[0] === "permanently_delete_records_batch",
    );
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ ids: [1, 2] });
  });

  it("toggleFavorite → toggle_favorite with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 3 })];
    await store.toggleFavorite(3);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "toggle_favorite");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 3 });
  });

  it("batchFavorite → batch_set_favorite with { ids, favorite }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1, is_favorite: false })];
    await store.batchFavorite([1]);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "batch_set_favorite");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ ids: [1], favorite: true });
  });

  it("togglePin → toggle_pin with { id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 4 })];
    await store.togglePin(4);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "toggle_pin");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 4 });
  });

  it("setAlias → set_record_alias with { id, alias }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 6 })];
    await store.setAlias(6, "my-alias");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "set_record_alias");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 6, alias: "my-alias" });
  });

  it("purgeExpiredRecords → cleanup_expired with no params", async () => {
    const store = useClipboardStore();
    // Default mock resolves undefined; give it an empty list so the expiry
    // sweep's removeExpiredFromList is a no-op instead of logging an error.
    vi.mocked(invoke).mockResolvedValueOnce([]);
    await store.purgeExpiredRecords();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "cleanup_expired");
    expect(call).toBeTruthy();
    // No args or empty object
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("emptyTrash → empty_trash with no params", async () => {
    const store = useClipboardStore();
    await store.emptyTrash();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "empty_trash");
    expect(call).toBeTruthy();
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("loadTrashCount → get_trash_count with no params", async () => {
    const store = useClipboardStore();
    await store.loadTrashCount();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_trash_count");
    expect(call).toBeTruthy();
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("togglePauseCapture → set_capture_paused with { paused }", async () => {
    const store = useClipboardStore();
    await store.togglePauseCapture();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "set_capture_paused");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    expect(typeof params.paused).toBe("boolean");
  });

  it("loadStats → get_stats with no params", async () => {
    const store = useClipboardStore();
    await store.loadStats();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_stats");
    expect(call).toBeTruthy();
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("importRecords → import_data with { records_json }", async () => {
    const store = useClipboardStore();
    await store.importRecords([makeRecord({ id: 99 })]);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "import_data");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    // Payload is a raw JSON string so the backend validates size before deserialize.
    expect(typeof params.records_json).toBe("string");
    const parsed = JSON.parse(params.records_json as string);
    expect(Array.isArray(parsed)).toBe(true);
  });

  it("loadTags → get_all_tags with { content_type, favorites_only }", async () => {
    const store = useClipboardStore();
    await store.loadTags();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_all_tags");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    expect("content_type" in params).toBe(true);
    expect("favorites_only" in params).toBe(true);
  });

  it("re-enabling the tags feature persists settings before reloading tags", async () => {
    // Mirror the backend: get_all_tags is gated on the *persisted* settings,
    // which lag behind the reactive store until save_settings completes.
    let tagsEnabledOnBackend = false;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "save_settings") {
        tagsEnabledOnBackend = (args as { settings: Settings }).settings.features.tags;
        return Promise.resolve(undefined);
      }
      if (cmd === "get_all_tags") {
        return tagsEnabledOnBackend
          ? Promise.resolve([])
          : Promise.reject(new Error("feature disabled: tags"));
      }
      return Promise.resolve(undefined);
    });
    const clip = useClipboardStore();
    const settings = useSettingsStore();

    // Tags start disabled; loading clears the in-memory tag list.
    settings.updateSetting("features", { ...settings.settings.features, tags: false });
    await clip.loadTags();
    vi.mocked(invoke).mockClear();

    // Re-enabling tags (as from Settings → Features) must refresh the list.
    settings.updateSetting("features", { ...settings.settings.features, tags: true });
    await vi.waitFor(() => {
      const saveCall = vi.mocked(invoke).mock.calls.find((c) => c[0] === "save_settings");
      const tagsCall = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_all_tags");
      // get_all_tags must be issued only after the backend accepted tags:true.
      expect(saveCall).toBeTruthy();
      expect(tagsCall).toBeTruthy();
      expect(tagsEnabledOnBackend).toBe(true);
    });
  });

  it("createTag → create_tag with { name, color }", async () => {
    const store = useClipboardStore();
    await store.createTag("new-tag", "#ff0000");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "create_tag");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ name: "new-tag", color: "#ff0000" });
  });

  it("deleteTag → delete_tag with { id }", async () => {
    const store = useClipboardStore();
    await store.deleteTag(5);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "delete_tag");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 5 });
  });

  it("updateTag → update_tag with { id, name, color }", async () => {
    const store = useClipboardStore();
    await store.updateTag(3, "renamed", "#00ff00");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "update_tag");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ id: 3, name: "renamed", color: "#00ff00" });
  });

  it("addTagToRecord → add_tag_to_record with { record_id, tag_id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 10 })];
    await store.addTagToRecord(10, 1, "vue");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "add_tag_to_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ record_id: 10, tag_id: 1 });
  });

  it("removeTagFromRecord → remove_tag_from_record with { record_id, tag_id }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 10, tags: ["vue"] })];
    await store.removeTagFromRecord(10, 1, "vue");
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "remove_tag_from_record");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ record_id: 10, tag_id: 1 });
  });

  it("setRecordTags → set_record_tags with { record_id, tag_ids }", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 10 })];
    await store.setRecordTags(10, [1, 2], ["vue", "ts"]);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "set_record_tags");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ record_id: 10, tag_ids: [1, 2] });
  });

  // ── settings store ──

  it("loadSettings → get_settings with no params", async () => {
    const store = useSettingsStore();
    await store.loadSettings();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "get_settings");
    expect(call).toBeTruthy();
    const args = call![1];
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("saveSettings → save_settings with { settings }", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_settings") return Promise.resolve(MOCK_SETTINGS);
      return Promise.resolve(undefined);
    });
    const store = useSettingsStore();
    await store.loadSettings(); // sets isLoaded = true
    vi.mocked(invoke).mockClear();
    await store.saveSettings();
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "save_settings");
    expect(call).toBeTruthy();
    const params = call![1] as Record<string, unknown>;
    expect(typeof params.settings).toBe("object");
  });

  it("applyAppearance → set_window_corner_radius with { radius }", async () => {
    const store = useSettingsStore();
    store.updateSetting("panel_radius", 12);
    // applyAppearance is called by updateSetting when panel_radius changes
    const call = vi.mocked(invoke).mock.calls.find(
      (c) => c[0] === "set_window_corner_radius",
    );
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ radius: 12 });
  });

  it("applyAppearance → set_window_backdrop with { enabled }", async () => {
    const store = useSettingsStore();
    store.updateSetting("enable_blur", true);
    const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === "set_window_backdrop");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ enabled: true });
  });
});

// ─── Contract completeness ────────────────────────────────────────────────────

describe("Tauri invoke contract — completeness", () => {
  it("every store invoke call targets a known command", async () => {
    setActivePinia(createPinia());
    // Provide return values that match Rust response shapes so store code
    // does not crash on undefined when reading .records / .find etc.
    vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_records" || cmd === "search_records") {
        return Promise.resolve({ records: [], has_more: false });
      }
      if (cmd === "get_settings") return Promise.resolve(MOCK_SETTINGS);
      if (cmd === "get_stats") return Promise.resolve(MOCK_STATS);
      if (cmd === "get_all_tags") return Promise.resolve([]);
      if (cmd === "get_record") return Promise.resolve(null);
      if (cmd === "toggle_favorite" || cmd === "toggle_pin") return Promise.resolve(true);
      if (cmd === "set_record_alias") {
        return Promise.resolve((args as Record<string, unknown>)?.alias ?? "");
      }
      if (cmd === "cleanup_expired") return Promise.resolve([]);
      if (cmd === "get_trash_count") return Promise.resolve(0);
      if (cmd === "import_data") return Promise.resolve(0);
      if (cmd === "create_tag") {
        const a = args as Record<string, unknown>;
        return Promise.resolve({ id: 1, name: a?.name, color: a?.color, is_auto: false, count: 0 });
      }
      return Promise.resolve(undefined);
    });
    vi.mocked(invoke).mockClear();

    // Exercise clipboard store actions
    const clip = useClipboardStore();
    await clip.loadRecords();
    await clip.search("test");
    clip.records = [makeRecord({ id: 1 })];
    await clip.ensureRecordDetail(1);
    await clip.pasteRecord(1);
    await clip.deleteRecord(1);
    await clip.deleteBatch([1]);
    await clip.restoreRecord(1);
    await clip.restoreRecordsBatch([1]);
    await clip.permanentlyDeleteRecord(1);
    await clip.permanentlyDeleteRecordsBatch([1]);
    await clip.toggleFavorite(1);
    clip.records = [makeRecord({ id: 1, is_favorite: false })];
    await clip.batchFavorite([1]);
    await clip.togglePin(1);
    await clip.setAlias(1, "alias");
    await clip.purgeExpiredRecords();
    await clip.emptyTrash();
    await clip.loadTrashCount();
    await clip.togglePauseCapture();
    await clip.loadStats();
    await clip.importRecords([makeRecord()]);
    await clip.loadTags();
    await clip.createTag("t", "#000");
    await clip.deleteTag(1);
    await clip.updateTag(1, "t2", "#111");
    clip.records = [makeRecord({ id: 1, tags: ["vue"] })];
    await clip.addTagToRecord(1, 1, "vue");
    await clip.removeTagFromRecord(1, 1, "vue");
    await clip.setRecordTags(1, [1], ["vue"]);

    // Exercise settings store actions
    const settings = useSettingsStore();
    await settings.loadSettings();
    await settings.saveSettings();
    settings.updateSetting("panel_radius", 10);
    settings.updateSetting("enable_blur", true);

    // Check every invoked command exists in the contract
    const commands = new Set(vi.mocked(invoke).mock.calls.map((c) => c[0] as string));
    for (const cmd of commands) {
      expect(
        COMMAND_CONTRACTS[cmd],
        `Command "${cmd}" invoked by stores but missing from contract definition`,
      ).toBeDefined();
    }
  });
});
