import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRecord, SearchResult, StatsData, Tag } from "../types";

export type FilterTab = 'all' | 'text' | 'code' | 'link' | 'image' | 'file' | 'favorites';

export const useClipboardStore = defineStore("clipboard", () => {
  // === State ===
  const records = ref<ClipboardRecord[]>([]);
  const selectedId = ref<number | null>(null);
  const isLoading = ref(false);
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

  // === Getters ===
  const selectedRecord = computed(() =>
    records.value.find((r) => r.id === selectedId.value) ?? null
  );

  const filteredRecords = computed(() => {
    let list = records.value;
    // In trash mode, show all trashed records without additional filtering
    if (trashFilter.value) return list;
    if (activeTag.value) {
      list = list.filter((r) => r.tags.includes(activeTag.value!));
    } else if (activeFilter.value === "favorites") {
      list = list.filter((r) => r.is_favorite);
    } else if (activeFilter.value !== "all") {
      list = list.filter((r) => r.content_type === activeFilter.value);
    }
    return list;
  });

  const filterCounts = computed(() => {
    let text = 0, code = 0, link = 0, image = 0, file = 0, favorites = 0;
    for (const r of records.value) {
      switch (r.content_type) {
        case "text": text++; break;
        case "code": code++; break;
        case "link": link++; break;
        case "image": image++; break;
        case "file": file++; break;
      }
      if (r.is_favorite) favorites++;
    }
    return {
      all: records.value.length,
      text, code, link, image, file, favorites,
    };
  });

  // === Actions ===
  async function loadRecords() {
    isLoading.value = true;
    try {
      const result = await invoke<ClipboardRecord[]>("get_records", { limit: 500, trashed: trashFilter.value });
      records.value = result;
      await loadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Failed to load records:", e);
    } finally {
      isLoading.value = false;
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
    isSearching.value = true;
    searchQuery.value = query;
    try {
      const result = await invoke<SearchResult>("search_records", { query });
      // Stale guard: discard response if a newer search was dispatched,
      // or if user has navigated to trash while search was in-flight
      if (capturedSeq !== searchSeq || trashFilter.value) {
        searchQuery.value = "";
        isSearching.value = false;
        return;
      }
      records.value = result.records;
    } catch (e) {
      console.error("Search failed:", e);
    } finally {
      isSearching.value = false;
    }
  }

  function selectRecord(id: number) {
    selectedId.value = id;
    if (!batchMode.value) {
      selectedIds.value.clear();
    }
  }

  function setFilter(filter: FilterTab) {
    activeFilter.value = filter;
    activeTag.value = null;
    selectedId.value = null;
  }

  async function pasteRecord(id: number, mode: "original" | "plain" = "original") {
    try {
      await invoke("paste_record", { id, mode });
      // Close panel after paste (handled by frontend)
    } catch (e) {
      console.error("Paste failed:", e);
    }
  }

  async function deleteRecord(id: number) {
    try {
      await invoke("delete_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (selectedId.value === id) selectedId.value = null;
      await loadStats();
    } catch (e) {
      console.error("Delete failed:", e);
    }
  }

  async function toggleFavorite(id: number) {
    const record = records.value.find((r) => r.id === id);
    if (!record) return;
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { id });
      record.is_favorite = newVal;
      await loadStats();
    } catch (e) {
      console.error("Toggle favorite failed:", e);
    }
  }

  async function togglePin(id: number) {
    const record = records.value.find((r) => r.id === id);
    if (!record) return;
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
    } catch (e) {
      console.error("Toggle pin failed:", e);
    }
  }

  async function deleteBatch(ids: number[]) {
    try {
      await invoke("delete_records_batch", { ids });
      records.value = records.value.filter((r) => !ids.includes(r.id));
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value.clear();
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
      selectedIds.value.clear();
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
    if (!batchMode.value) selectedIds.value.clear();
  }

  function toggleBatchSelect(id: number) {
    if (selectedIds.value.has(id)) {
      selectedIds.value.delete(id);
    } else {
      selectedIds.value.add(id);
    }
  }

  async function togglePauseCapture() {
    pauseCapture.value = !pauseCapture.value;
    try {
      await invoke("set_capture_paused", { paused: pauseCapture.value });
    } catch (e) {
      console.error("Toggle pause failed:", e);
    }
  }

  // Called by event listener when clipboard changes
  function onNewRecord(record: ClipboardRecord) {
    // Remove existing record with same id (backend dedup returns same id on hash match)
    const existing = records.value.findIndex((r) => r.id === record.id);
    if (existing !== -1) {
      records.value.splice(existing, 1);
    }
    // Insert at top, after pinned items
    const pinCount = records.value.filter((r) => r.is_pinned).length;
    records.value.splice(pinCount, 0, record);
    loadStats().catch(() => {}); // fire-and-forget in hot path
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
      await invoke("delete_tag", { id });
      await loadTags();
    } catch (e) {
      console.error("Failed to delete tag:", e);
    }
  }

  async function updateTag(id: number, name: string, color: string) {
    try {
      await invoke("update_tag", { id, name, color });
      await loadTags();
    } catch (e) {
      console.error("Failed to update tag:", e);
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
    activeTag.value = tagName;
    if (tagName) {
      activeFilter.value = "all";
    }
    selectedId.value = null;
  }

  return {
    // State
    records,
    selectedId,
    isLoading,
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
    search,
    selectRecord,
    setFilter,
    pasteRecord,
    deleteRecord,
    toggleFavorite,
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
    togglePauseCapture,
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
