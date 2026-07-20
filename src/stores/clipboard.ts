import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRecord, RecordsPage, SearchResult, StatsData, Tag } from "../types";

export type FilterTab = 'all' | 'text' | 'code' | 'link' | 'image' | 'file' | 'favorites';

const PAGE_SIZE = 60;

export const useClipboardStore = defineStore("clipboard", () => {
  // === State ===
  const records = ref<ClipboardRecord[]>([]);
  const selectedId = ref<number | null>(null);
  const isLoading = ref(false);
  const isLoadingMore = ref(false);
  const hasMore = ref(true);
  const isSearching = ref(false);
  const searchQuery = ref("");
  const activeFilter = ref<FilterTab>("all");
  const activeTag = ref<string | null>(null);
  const trashFilter = ref(false);
  const trashCount = ref(0);
  const batchMode = ref(false);
  const selectedIds = ref<Set<number>>(new Set());
  const pauseCapture = ref(false);
  const stats = ref<StatsData | null>(null);
  const tags = ref<Tag[]>([]);
  let searchSeq = 0;
  let loadSeq = 0;

  // === Getters ===
  const selectedRecord = computed(() =>
    records.value.find((r) => r.id === selectedId.value) ?? null
  );

  /** Server applies category/tag/trash/search filters; list is ready to render. */
  const filteredRecords = computed(() => records.value);

  const filterCounts = computed(() => {
    const dist = (stats.value?.type_distribution ?? {}) as Record<string, number>;
    const n = (key: string) => Number(dist[key] ?? 0);
    return {
      all: stats.value?.total_records ?? 0,
      text: n("text"),
      code: n("code"),
      link: n("link"),
      image: n("image"),
      file: n("file"),
      favorites: stats.value?.favorites_count ?? 0,
    };
  });

  function listQueryArgs(offset: number) {
    const favoritesOnly = !trashFilter.value && activeFilter.value === "favorites";
    // Must match #[tauri::command(rename_all = "snake_case")] on get_records.
    return {
      limit: PAGE_SIZE,
      offset,
      trashed: trashFilter.value,
      content_type:
        !trashFilter.value && !favoritesOnly && activeFilter.value !== "all"
          ? activeFilter.value
          : null,
      favorites_only: favoritesOnly,
      tag: !trashFilter.value ? activeTag.value : null,
    };
  }

  function searchFilterArgs() {
    const favoritesOnly = activeFilter.value === "favorites";
    // Must match #[tauri::command(rename_all = "snake_case")] on search_records.
    return {
      content_type:
        !favoritesOnly && activeFilter.value !== "all" ? activeFilter.value : null,
      favorites_only: favoritesOnly,
      tag: activeTag.value,
    };
  }

  function reloadList() {
    if (searchQuery.value.trim()) {
      void search(searchQuery.value);
    } else {
      void loadRecords();
    }
  }

  function appendRecords(batch: ClipboardRecord[]) {
    const seen = new Set(records.value.map((r) => r.id));
    for (const r of batch) {
      if (!seen.has(r.id)) {
        records.value.push(r);
        seen.add(r.id);
      }
    }
  }

  // === Actions ===
  async function loadRecords() {
    const seq = ++loadSeq;
    isLoading.value = true;
    isLoadingMore.value = false;
    hasMore.value = true;
    try {
      const page = await invoke<RecordsPage>("get_records", listQueryArgs(0));
      if (seq !== loadSeq) return;
      records.value = page.records;
      hasMore.value = page.has_more;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Failed to load records:", e);
    } finally {
      if (seq === loadSeq) isLoading.value = false;
    }
  }

  async function loadMore() {
    if (!hasMore.value || isLoading.value || isLoadingMore.value) return;
    const seq = loadSeq;
    isLoadingMore.value = true;
    try {
      const offset = records.value.length;
      if (searchQuery.value.trim()) {
        const result = await invoke<SearchResult>("search_records", {
          query: searchQuery.value,
          limit: PAGE_SIZE,
          offset,
          ...searchFilterArgs(),
        });
        if (seq !== loadSeq || trashFilter.value) return;
        appendRecords(result.records);
        hasMore.value = result.has_more;
      } else {
        const page = await invoke<RecordsPage>("get_records", listQueryArgs(offset));
        if (seq !== loadSeq) return;
        appendRecords(page.records);
        hasMore.value = page.has_more;
      }
    } catch (e) {
      console.error("Failed to load more records:", e);
    } finally {
      if (seq === loadSeq) isLoadingMore.value = false;
    }
  }

  async function search(query: string) {
    if (!query.trim()) {
      searchQuery.value = "";
      isSearching.value = false;
      await loadRecords();
      return;
    }
    const capturedSeq = ++searchSeq;
    ++loadSeq;
    const seq = loadSeq;
    isSearching.value = true;
    isLoading.value = true;
    searchQuery.value = query;
    hasMore.value = true;
    try {
      const result = await invoke<SearchResult>("search_records", {
        query,
        limit: PAGE_SIZE,
        offset: 0,
        ...searchFilterArgs(),
      });
      // Stale guard: discard response if a newer search was dispatched,
      // or if user has navigated to trash while search was in-flight
      if (capturedSeq !== searchSeq || trashFilter.value || seq !== loadSeq) {
        return;
      }
      records.value = result.records;
      hasMore.value = result.has_more;
    } catch (e) {
      console.error("Search failed:", e);
    } finally {
      if (capturedSeq === searchSeq) {
        isSearching.value = false;
        isLoading.value = false;
      }
    }
  }

  function selectRecord(id: number) {
    selectedId.value = id;
    if (!batchMode.value) {
      selectedIds.value = new Set();
    }
    void ensureRecordDetail(id);
  }

  function clearSelection() {
    selectedId.value = null;
  }

  /** Lazy-load content_html for preview when list rows omit it. */
  async function ensureRecordDetail(id: number) {
    const record = records.value.find((r) => r.id === id);
    if (!record || record.content_html != null || record.content_type === "image") return;
    try {
      const full = await invoke<ClipboardRecord | null>("get_record", { id });
      if (!full) return;
      const idx = records.value.findIndex((r) => r.id === id);
      if (idx !== -1) {
        records.value[idx] = { ...records.value[idx], ...full, tags: records.value[idx].tags };
      }
    } catch (e) {
      console.error("Failed to load record detail:", e);
    }
  }

  function setFilter(filter: FilterTab) {
    activeFilter.value = filter;
    // Keep activeTag — type/favorites and tag combine with AND.
    selectedId.value = null;
    reloadList();
  }

  async function pasteRecord(id: number, mode: "original" | "plain" = "original") {
    try {
      await invoke("paste_record", { id, mode });
    } catch (e) {
      console.error("Paste failed:", e);
      throw e;
    }
  }

  /** Set favorite on for all ids that are not already favorited. */
  async function batchFavorite(ids: number[]) {
    const toFav = ids.filter((id) => {
      const r = records.value.find((x) => x.id === id);
      return r && !r.is_favorite;
    });
    if (!toFav.length) return;
    try {
      await invoke("batch_set_favorite", { ids: toFav, favorite: true });
      for (const id of toFav) {
        const record = records.value.find((r) => r.id === id);
        if (record) record.is_favorite = true;
      }
      await loadStats();
    } catch (e) {
      console.error("Batch favorite failed:", e);
    }
  }

  async function deleteRecord(id: number) {
    try {
      await invoke("delete_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (selectedId.value === id) selectedId.value = null;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Delete failed:", e);
    }
  }

  async function toggleFavorite(id: number): Promise<boolean | null> {
    const record = records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { id });
      record.is_favorite = newVal;
      await loadStats();
      return newVal;
    } catch (e) {
      console.error("Toggle favorite failed:", e);
      return null;
    }
  }

  async function togglePin(id: number): Promise<boolean | null> {
    const record = records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const newVal = await invoke<boolean>("toggle_pin", { id });
      record.is_pinned = newVal;
      // Re-sort to bring pinned to top
      records.value.sort((a, b) => {
        if (a.is_pinned && !b.is_pinned) return -1;
        if (!a.is_pinned && b.is_pinned) return 1;
        return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
      });
      await loadStats();
      return newVal;
    } catch (e) {
      console.error("Toggle pin failed:", e);
      return null;
    }
  }

  async function deleteBatch(ids: number[]) {
    try {
      await invoke("delete_records_batch", { ids });
      records.value = records.value.filter((r) => !ids.includes(r.id));
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value = new Set();
      batchMode.value = false;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Batch delete failed:", e);
    }
  }

  // === Trash / Restore ===

  async function restoreRecord(id: number) {
    try {
      await invoke("restore_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (selectedId.value === id) selectedId.value = null;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Restore failed:", e);
    }
  }

  async function restoreRecordsBatch(ids: number[]) {
    try {
      await invoke("restore_records_batch", { ids });
      records.value = records.value.filter((r) => !ids.includes(r.id));
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value = new Set();
      batchMode.value = false;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Batch restore failed:", e);
    }
  }

  async function permanentlyDeleteRecord(id: number) {
    try {
      await invoke("permanently_delete_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (selectedId.value === id) selectedId.value = null;
      await loadTrashCount();
    } catch (e) {
      console.error("Permanent delete failed:", e);
    }
  }

  async function emptyTrash() {
    try {
      await invoke("empty_trash");
      records.value = [];
      selectedId.value = null;
      trashCount.value = 0;
      await loadStats();
    } catch (e) {
      console.error("Empty trash failed:", e);
    }
  }

  async function loadTrashCount() {
    try {
      trashCount.value = await invoke<number>("get_trash_count");
    } catch (e) {
      console.error("Load trash count failed:", e);
    }
  }

  function setTrashFilter(on: boolean) {
    trashFilter.value = on;
    if (on) {
      activeTag.value = null;
      activeFilter.value = "all";
      searchQuery.value = "";
      isSearching.value = false;
    }
    selectedId.value = null;
  }

  function toggleBatchMode() {
    batchMode.value = !batchMode.value;
    if (!batchMode.value) selectedIds.value = new Set();
  }

  function toggleBatchSelect(id: number) {
    const next = new Set(selectedIds.value);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds.value = next;
  }

  function setPauseCapture(paused: boolean) {
    pauseCapture.value = paused;
  }

  async function togglePauseCapture() {
    const next = !pauseCapture.value;
    pauseCapture.value = next;
    try {
      await invoke("set_capture_paused", { paused: next });
    } catch (e) {
      pauseCapture.value = !next;
      console.error("Toggle pause failed:", e);
    }
  }

  // Called by event listener when clipboard changes
  function onNewRecord(record: ClipboardRecord) {
    loadStats().catch(() => {}); // fire-and-forget in hot path
    if (trashFilter.value || searchQuery.value) return;
    if (activeTag.value && !record.tags.includes(activeTag.value)) return;
    if (activeFilter.value === "favorites" && !record.is_favorite) return;
    if (
      activeFilter.value !== "all" &&
      activeFilter.value !== "favorites" &&
      record.content_type !== activeFilter.value
    ) {
      return;
    }
    // Remove existing record with same id (backend dedup returns same id on hash match)
    const existing = records.value.findIndex((r) => r.id === record.id);
    if (existing !== -1) {
      records.value.splice(existing, 1);
    }
    // Insert at top, after pinned items
    const pinCount = records.value.filter((r) => r.is_pinned).length;
    records.value.splice(pinCount, 0, record);
  }

  async function loadStats() {
    try {
      stats.value = await invoke<StatsData>("get_stats");
    } catch (e) {
      console.error("Load stats failed:", e);
    }
  }

  async function importRecords(importedRecords: ClipboardRecord[]) {
    const imported = await invoke<number>("import_data", { records: importedRecords });
    await loadRecords();
    return imported;
  }

  // === Tag Actions ===

  async function loadTags() {
    try {
      tags.value = await invoke<Tag[]>("get_all_tags");
    } catch (e) {
      console.error("Failed to load tags:", e);
    }
  }

  async function createTag(name: string, color: string) {
    try {
      await invoke<Tag>("create_tag", { name, color });
      await loadTags();
    } catch (e) {
      console.error("Failed to create tag:", e);
    }
  }

  async function deleteTag(id: number) {
    try {
      const existing = tags.value.find((t) => t.id === id);
      await invoke("delete_tag", { id });
      if (existing) {
        for (const record of records.value) {
          if (record.tags.includes(existing.name)) {
            record.tags = record.tags.filter((t) => t !== existing.name);
          }
        }
        if (activeTag.value === existing.name) {
          activeTag.value = null;
        }
      }
      await loadTags();
    } catch (e) {
      console.error("Failed to delete tag:", e);
      throw e;
    }
  }

  async function updateTag(id: number, name: string, color: string) {
    try {
      const existing = tags.value.find((t) => t.id === id);
      const oldName = existing?.name;
      await invoke("update_tag", { id, name, color });
      if (oldName && oldName !== name) {
        for (const record of records.value) {
          const idx = record.tags.indexOf(oldName);
          if (idx !== -1) {
            const next = [...record.tags];
            next[idx] = name;
            record.tags = next;
          }
        }
        if (activeTag.value === oldName) {
          activeTag.value = name;
        }
      }
      await loadTags();
    } catch (e) {
      console.error("Failed to update tag:", e);
      throw e;
    }
  }

  async function addTagToRecord(recordId: number, tagId: number, tagName: string) {
    try {
      await invoke("add_tag_to_record", { recordId, tagId });
      const record = records.value.find((r) => r.id === recordId);
      if (record && !record.tags.includes(tagName)) {
        record.tags = [...record.tags, tagName];
      }
      await loadTags();
    } catch (e) {
      console.error("Failed to add tag to record:", e);
    }
  }

  async function removeTagFromRecord(recordId: number, tagId: number, tagName: string) {
    try {
      await invoke("remove_tag_from_record", { recordId, tagId });
      const record = records.value.find((r) => r.id === recordId);
      if (record) {
        record.tags = record.tags.filter((t) => t !== tagName);
      }
      await loadTags();
    } catch (e) {
      console.error("Failed to remove tag from record:", e);
    }
  }

  function filterByTag(tagName: string | null) {
    // Toggle off when clicking the same tag again; keep type/favorites filter.
    if (tagName && activeTag.value === tagName) {
      activeTag.value = null;
    } else {
      activeTag.value = tagName;
    }
    selectedId.value = null;
    reloadList();
  }

  return {
    // State
    records,
    selectedId,
    isLoading,
    isLoadingMore,
    hasMore,
    isSearching,
    searchQuery,
    activeFilter,
    activeTag,
    trashFilter,
    trashCount,
    batchMode,
    selectedIds,
    pauseCapture,
    stats,
    tags,
    // Getters
    selectedRecord,
    filteredRecords,
    filterCounts,
    // Actions
    loadRecords,
    loadMore,
    search,
    selectRecord,
    clearSelection,
    setFilter,
    reloadList,
    pasteRecord,
    deleteRecord,
    toggleFavorite,
    batchFavorite,
    togglePin,
    deleteBatch,
    restoreRecord,
    restoreRecordsBatch,
    permanentlyDeleteRecord,
    emptyTrash,
    loadTrashCount,
    setTrashFilter,
    toggleBatchMode,
    toggleBatchSelect,
    setPauseCapture,
    togglePauseCapture,
    ensureRecordDetail,
    onNewRecord,
    loadStats,
    importRecords,
    loadTags,
    createTag,
    deleteTag,
    updateTag,
    addTagToRecord,
    removeTagFromRecord,
    filterByTag,
  };
});
