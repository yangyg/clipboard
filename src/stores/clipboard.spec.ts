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

describe("clipboardStore — onNewRecord filtering", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("skips prepend when trash filter is active", () => {
    const store = useClipboardStore();
    store.setTrashFilter(true);
    store.onNewRecord(makeRecord({ id: 10 }));
    expect(store.records).toEqual([]);
  });

  it("skips prepend when search query is active", () => {
    const store = useClipboardStore();
    store.searchQuery = "hello"; // simulate active search
    store.onNewRecord(makeRecord({ id: 11 }));
    expect(store.records).toEqual([]);
  });

  it("skips when activeTag does not match record tags", () => {
    const store = useClipboardStore();
    store.filterByTag("vue"); // sets activeTag to "vue"
    store.onNewRecord(makeRecord({ id: 12, tags: ["react"] }));
    expect(store.records).toEqual([]);
  });

  it("allows record when activeTag matches", () => {
    const store = useClipboardStore();
    store.filterByTag("vue");
    store.onNewRecord(makeRecord({ id: 13, tags: ["vue", "ts"] }));
    expect(store.records.map((r) => r.id)).toEqual([13]);
  });

  it("skips non-favorite record when filter is 'favorites'", () => {
    const store = useClipboardStore();
    store.setFilter("favorites");
    store.onNewRecord(makeRecord({ id: 14, is_favorite: false }));
    expect(store.records).toEqual([]);
  });

  it("allows favorite record when filter is 'favorites'", () => {
    const store = useClipboardStore();
    store.setFilter("favorites");
    store.onNewRecord(makeRecord({ id: 15, is_favorite: true }));
    expect(store.records.map((r) => r.id)).toEqual([15]);
  });

  it("skips record with mismatched content_type filter", () => {
    const store = useClipboardStore();
    store.setFilter("code");
    store.onNewRecord(makeRecord({ id: 16, content_type: "text" }));
    expect(store.records).toEqual([]);
  });

  it("allows record with matching content_type filter", () => {
    const store = useClipboardStore();
    store.setFilter("link");
    store.onNewRecord(makeRecord({ id: 17, content_type: "link" }));
    expect(store.records.map((r) => r.id)).toEqual([17]);
  });
});

describe("clipboardStore — removeExpiredFromList", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("removes specified ids from records", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 }), makeRecord({ id: 3 })];
    store.removeExpiredFromList([1, 3]);
    expect(store.records.map((r) => r.id)).toEqual([2]);
  });

  it("clears selectedId if it was expired", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 5 })];
    store.selectedId = 5;
    store.removeExpiredFromList([5]);
    expect(store.selectedId).toBeNull();
  });

  it("keeps selectedId if not in expired set", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 5 }), makeRecord({ id: 6 })];
    store.selectedId = 5;
    store.removeExpiredFromList([6]);
    expect(store.selectedId).toBe(5);
  });

  it("is a no-op for empty ids array", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 })];
    store.removeExpiredFromList([]);
    expect(store.records.length).toBe(1);
  });
});

describe("clipboardStore — selectedRecord computed", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("returns null when no record is selected", () => {
    const store = useClipboardStore();
    expect(store.selectedRecord).toBeNull();
  });

  it("returns the base record when no detail is loaded", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1, content: "base" })];
    store.selectedId = 1;
    expect(store.selectedRecord?.content).toBe("base");
  });
});

describe("clipboardStore — patchRecord & filterByTag", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("onNewRecord deduplicates existing record by moving it to front", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 })];
    // Re-insert id=1 with updated content
    store.onNewRecord(makeRecord({ id: 1, content: "updated" }));
    // id=1 should be at front (no pinned rows), id=2 stays
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
    expect(store.records[0].content).toBe("updated");
  });

  it("filterByTag toggles off when clicking the same tag", () => {
    const store = useClipboardStore();
    store.filterByTag("vue");
    expect(store.activeTag).toBe("vue");
    store.filterByTag("vue");
    expect(store.activeTag).toBeNull();
  });

  it("filterByTag switches to a different tag", () => {
    const store = useClipboardStore();
    store.filterByTag("vue");
    store.filterByTag("react");
    expect(store.activeTag).toBe("react");
  });
});

describe("clipboardStore — setTrashFilter side effects", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("clears activeTag, activeFilter, and searchQuery when enabling trash", () => {
    const store = useClipboardStore();
    store.filterByTag("vue");
    store.searchQuery = "test";
    store.setTrashFilter(true);
    expect(store.activeTag).toBeNull();
    expect(store.activeFilter).toBe("all");
    expect(store.searchQuery).toBe("");
    expect(store.isSearching).toBe(false);
    expect(store.selectedId).toBeNull();
  });
});

describe("clipboardStore — reorderForUpdate / reorderForUpdates (tag-edit re-rank)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("moves a non-pinned mid-list record to the front of the unpinned section", () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 10, is_pinned: true }),
      makeRecord({ id: 1 }),
      makeRecord({ id: 2 }),
      makeRecord({ id: 3 }),
    ];
    store.reorderForUpdate(2);
    expect(store.records.map((r) => r.id)).toEqual([10, 2, 1, 3]);
  });

  it("moves a pinned record to the top of the pinned block", () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 2, is_pinned: true }),
      makeRecord({ id: 1, is_pinned: true }),
      makeRecord({ id: 3 }),
    ];
    store.reorderForUpdate(1);
    expect(store.records.map((r) => r.id)).toEqual([1, 2, 3]);
  });

  it("is a no-op for ids not in the current window", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 })];
    store.reorderForUpdates([99, 42]);
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
  });

  it("leaves rows untouched for non-updated_desc sorts (reload is deferred)", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 }), makeRecord({ id: 3 })];
    store.listSort = "created_desc";
    store.reorderForUpdate(2);
    expect(store.records.map((r) => r.id)).toEqual([1, 2, 3]);
  });

  it("skips re-rank while a tag filter the record no longer matches is active", () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, tags: ["vue"] }),
      makeRecord({ id: 2, tags: ["react"] }),
    ];
    store.activeTag = "vue";
    store.reorderForUpdate(2);
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
  });

  it("skips re-rank while trash filter is active", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 })];
    store.setTrashFilter(true);
    store.reorderForUpdate(2);
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
  });

  it("reorders a batch the way the server tie-breaks equal timestamps (id DESC)", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 }), makeRecord({ id: 3 })];
    store.reorderForUpdates([1, 2]);
    expect(store.records.map((r) => r.id)).toEqual([2, 1, 3]);
  });

  it("addTagToRecord re-ranks a record to the front after a real change", async () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, tags: ["a"] }),
      makeRecord({ id: 2, tags: [] }),
      makeRecord({ id: 3, tags: [] }),
    ];
    await store.addTagToRecord(2, 7, "b");
    expect(store.records.map((r) => r.id)).toEqual([2, 1, 3]);
    expect(store.records[0].tags).toContain("b");
  });

  it("addTagToRecord does not re-rank when the tag was already present", async () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, tags: [] }),
      makeRecord({ id: 2, tags: ["b"] }),
    ];
    await store.addTagToRecord(2, 7, "b");
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
  });

  it("setRecordTags re-ranks when the tag set changed", async () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, tags: ["a"] }),
      makeRecord({ id: 2, tags: ["keep"] }),
    ];
    await store.setRecordTags(2, [7, 8], ["b", "c"]);
    expect(store.records.map((r) => r.id)).toEqual([2, 1]);
    expect(store.records[0].tags).toEqual(["b", "c"]);
  });

  it("setRecordTags does not re-rank when the tag set is unchanged", async () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, tags: ["b"] }),
      makeRecord({ id: 2, tags: [] }),
    ];
    await store.setRecordTags(1, [7], ["b"]);
    expect(store.records.map((r) => r.id)).toEqual([1, 2]);
  });

  it("updateTag rename re-ranks every affected row", async () => {
    const store = useClipboardStore();
    store.tags = [{ id: 1, name: "old", color: "#fff", is_auto: false, count: 2 }];
    store.records = [
      makeRecord({ id: 1, tags: ["old"] }),
      makeRecord({ id: 2, tags: ["old"] }),
      makeRecord({ id: 3, tags: ["other"] }),
    ];
    await store.updateTag(1, "new", "#eee");
    // Both re-ranked; equal bump timestamps → server orders them id DESC.
    expect(store.records.map((r) => r.id)).toEqual([2, 1, 3]);
    expect(store.records[0].tags).toEqual(["new"]);
  });
});
