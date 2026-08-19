import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "@/stores/clipboard";
import { makeRecord } from "@/test/factories";
import type { StatsData } from "@/types";

describe("clipboardStore (smoke)", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
    setActivePinia(createPinia());
  });

  it("starts with empty, non-trash, all-filter defaults", () => {
    const store = useClipboardStore();
    expect(store.records).toEqual([]);
    expect(store.activeFilter).toBe("all");
    expect(store.trashFilter).toBe(false);
    expect(store.listSort).toBe("updated_desc");
    expect(store.pinnedCollapsed).toBe(false);
    expect(store.selectedId).toBeNull();
  });

  it("togglePinnedCollapsed persists across store recreations", () => {
    const store = useClipboardStore();
    store.togglePinnedCollapsed();
    expect(store.pinnedCollapsed).toBe(true);
    expect(localStorage.getItem("clipvault-pinned-collapsed")).toBe("1");

    setActivePinia(createPinia());
    const next = useClipboardStore();
    expect(next.pinnedCollapsed).toBe(true);
  });

  it("unfolds the pinned section after a successful pin", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 })];
    store.setPinnedCollapsed(true);
    vi.mocked(invoke).mockResolvedValueOnce(true);
    await store.togglePin(1);
    expect(store.pinnedCollapsed).toBe(false);
  });

  it("keeps the pinned section folded when unpinning", async () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1, is_pinned: true })];
    store.setPinnedCollapsed(true);
    vi.mocked(invoke).mockResolvedValueOnce(false);
    await store.togglePin(1);
    expect(store.pinnedCollapsed).toBe(true);
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

  it("selectAllFiltered selects every loaded row in batch mode", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 })];
    store.toggleBatchMode();
    store.selectAllFiltered();
    expect([...store.selectedIds].sort()).toEqual([1, 2]);
    store.clearBatchSelection();
    expect(store.selectedIds.size).toBe(0);
  });

  it("selectBatchRange selects an inclusive span from the last click", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 }), makeRecord({ id: 2 }), makeRecord({ id: 3 })];
    store.toggleBatchMode();
    store.toggleBatchSelect(1);
    store.selectBatchRange(3);
    expect([...store.selectedIds].sort()).toEqual([1, 2, 3]);
  });

  it("selectAllFiltered is a no-op outside batch mode", () => {
    const store = useClipboardStore();
    store.records = [makeRecord({ id: 1 })];
    store.selectAllFiltered();
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
    localStorage.removeItem("clipvault-pinned-collapsed");
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
    localStorage.removeItem("clipvault-pinned-collapsed");
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

describe("clipboardStore — purgeExpiredRecords respects favorite/pinned", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
    setActivePinia(createPinia());
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "cleanup_expired") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it("drops past-due unprotected rows but keeps favorited/pinned/trashed ones", async () => {
    const store = useClipboardStore();
    const past = "2000-01-01T00:00:00Z";
    store.records = [
      makeRecord({ id: 1, auto_expire_at: past }),
      makeRecord({ id: 2, auto_expire_at: past, is_favorite: true }),
      makeRecord({ id: 3, auto_expire_at: past, is_pinned: true }),
      makeRecord({ id: 4, auto_expire_at: past, is_trashed: true }),
    ];

    await store.purgeExpiredRecords();

    expect(store.records.map((r) => r.id)).toEqual([2, 3, 4]);
  });

  it("does not re-trigger the sweep loop over protected past-due rows", async () => {
    const store = useClipboardStore();
    store.records = [
      makeRecord({ id: 1, auto_expire_at: "2000-01-01T00:00:00Z", is_favorite: true }),
    ];

    await store.purgeExpiredRecords();

    expect(store.records.map((r) => r.id)).toEqual([1]);
    expect(
      vi.mocked(invoke).mock.calls.filter((c) => c[0] === "cleanup_expired"),
    ).toHaveLength(1);
  });
});

describe("clipboardStore — selectedRecord computed", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
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
    localStorage.removeItem("clipvault-pinned-collapsed");
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
    localStorage.removeItem("clipvault-pinned-collapsed");
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
    localStorage.removeItem("clipvault-pinned-collapsed");
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

  it("updateTag rename patches names without re-ranking (tags sync standalone)", async () => {
    const store = useClipboardStore();
    store.tags = [{ id: 1, name: "old", color: "#fff", is_auto: false, count: 2 }];
    store.records = [
      makeRecord({ id: 1, tags: ["old"] }),
      makeRecord({ id: 2, tags: ["old"] }),
      makeRecord({ id: 3, tags: ["other"] }),
    ];
    await store.updateTag(1, "new", "#eee");
    // Renames no longer bump records.updated_at (tag definitions sync via
    // tags.json), so the list order is untouched.
    expect(store.records.map((r) => r.id)).toEqual([1, 2, 3]);
    expect(store.records.map((r) => r.tags)).toEqual([
      ["new"],
      ["new"],
      ["other"],
    ]);
  });
});

describe("clipboardStore — parallel first-screen load", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
    setActivePinia(createPinia());
  });

  it("loads records, trash count, and stats concurrently and applies all results", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_records") {
        return { records: [makeRecord({ id: 42 })], has_more: false };
      }
      if (cmd === "get_trash_count") return 7;
      if (cmd === "get_stats") {
        return {
          total_records: 1,
          total_copies: 0,
          favorites_count: 0,
          pinned_count: 0,
          sensitive_count: 0,
          storage_bytes: 0,
          data_path: "",
          type_distribution: { text: 1 },
        } as StatsData;
      }
      return undefined;
    });

    const store = useClipboardStore();
    await store.loadRecords();

    expect(store.records.map((r) => r.id)).toEqual([42]);
    expect(store.trashCount).toBe(7);
    expect(store.stats?.total_records).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith("get_records", expect.anything());
    expect(invokeMock).toHaveBeenCalledWith("get_trash_count");
    expect(invokeMock).toHaveBeenCalledWith("get_stats");
  });

  it("keeps the record list even when the auxiliary stats call fails", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_records") {
        return { records: [makeRecord({ id: 1 })], has_more: false };
      }
      if (cmd === "get_trash_count") return 0;
      if (cmd === "get_stats") throw new Error("stats boom");
      return undefined;
    });

    const store = useClipboardStore();
    await store.loadRecords();
    expect(store.records.map((r) => r.id)).toEqual([1]);
  });
});

describe("clipboardStore — keyset loadMore", () => {
  beforeEach(() => {
    localStorage.removeItem("clipvault-pinned-collapsed");
    setActivePinia(createPinia());
  });

  it("sends a sort-key cursor for created_desc instead of OFFSET", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_records") {
        return { records: [makeRecord({ id: 11 })], has_more: false };
      }
      return undefined;
    });

    const store = useClipboardStore();
    store.listSort = "created_desc";
    store.records = [
      makeRecord({
        id: 10,
        created_at: "2026-02-01T00:00:00Z",
        updated_at: "2026-02-02T00:00:00Z",
        copy_count: 3,
      }),
    ];
    store.hasMore = true;
    await store.loadMore();

    expect(invokeMock).toHaveBeenCalledWith(
      "get_records",
      expect.objectContaining({
        offset: 0,
        sort: "created_desc",
        before_id: 10,
        before_created_at: "2026-02-01T00:00:00Z",
        before_updated_at: "2026-02-02T00:00:00Z",
        before_copy_count: 3,
      }),
    );
  });
});

