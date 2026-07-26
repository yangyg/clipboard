import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useClipboardStore } from "@/stores/clipboard";
import type { ClipboardRecord, StatsData } from "@/types";

function makeRecord(overrides: Partial<ClipboardRecord> = {}): ClipboardRecord {
  return {
    id: 1,
    content: "hello",
    content_type: "text",
    source_app: "test.exe",
    source_window: "Test",
    hash: "abc",
    copy_count: 0,
    is_favorite: false,
    is_pinned: false,
    is_sensitive: false,
    is_trashed: false,
    auto_expire_at: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    tags: [],
    alias: "",
    ...overrides,
  };
}

describe("clipboardStore (smoke)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("starts with empty, non-trash, all-filter defaults", () => {
    const store = useClipboardStore();
    expect(store.records).toEqual([]);
    expect(store.activeFilter).toBe("all");
    expect(store.trashFilter).toBe(false);
    expect(store.listSort).toBe("updated_desc");
    expect(store.selectedId).toBeNull();
  });

  it("toggles batch mode and clears selection when leaving it", () => {
    const store = useClipboardStore();
    store.toggleBatchMode();
    expect(store.batchMode).toBe(true);
    store.toggleBatchSelect(5);
    expect(store.selectedIds.has(5)).toBe(true);
    store.toggleBatchMode();
    expect(store.batchMode).toBe(false);
    expect(store.selectedIds.size).toBe(0);
  });

  it("prepends a new record after pinned rows via onNewRecord", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1, is_pinned: true })];
    store.onNewRecord(makeRecord({ id: 2 }));
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
    store.onNewRecord(makeRecord({ id: 3 }));
    // Newest non-pinned goes right after the pinned row.
    expect(store.records.map((r) => r.id)).toEqual([1, 3, 2]);
  });

  it("derives filterCounts from stats type distribution", () => {
    const store = useClipboardStore();
    store.stats = {
      total_records: 10,
      total_copies: 20,
      favorites_count: 3,
      pinned_count: 1,
      sensitive_count: 0,
      storage_bytes: 0,
      data_path: "",
      type_distribution: { text: 5, code: 2, link: 1, image: 2, file: 0, sensitive: 0 },
    } as StatsData;
    expect(store.filterCounts.all).toBe(10);
    expect(store.filterCounts.text).toBe(5);
    expect(store.filterCounts.image).toBe(2);
    expect(store.filterCounts.favorites).toBe(3);
  });
});
