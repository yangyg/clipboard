import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRecord, StatsData, Tag } from "../types";
import { featureEnabled } from "../composables/useFeature";
import { createTagActions } from "./clipboardTagActions";
import { createExpiryScheduler } from "./clipboardExpiry";
import {
  createListActions,
  type FilterTab,
  type ListSort,
} from "./clipboardList";
import { createRecordActions } from "./clipboardRecordActions";
import { useSettingsStore } from "./settings";
import { useToast } from "../composables/useToast";
import { i18n } from "../locales";
import { humanizeInvokeError } from "../utils/invokeError";
import {
  persistPinnedCollapsed,
  readPinnedCollapsed,
} from "../utils/pinnedList";

export type { FilterTab, ListSort } from "./clipboardList";
export { LIST_SORT_OPTIONS } from "./clipboardList";

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
  const listSort = ref<ListSort>("updated_desc");
  /** Fold the middle-column pinned group. Persisted in localStorage. */
  const pinnedCollapsed = ref(readPinnedCollapsed());
  const batchMode = ref(false);
  const selectedIds = ref<Set<number>>(new Set());
  /** Last row clicked in batch mode — Shift+click range starts here. */
  const batchAnchorId = ref<number | null>(null);
  const pauseCapture = ref(false);
  const stats = ref<StatsData | null>(null);
  const tags = ref<Tag[]>([]);
  /** Full content/HTML for preview only — never merge back into list rows. */
  const recordDetails = ref<Map<number, ClipboardRecord>>(new Map());
  /** Freshly captured record id — drives the row-flash highlight in the list. */
  const lastIncomingId = ref<number | null>(null);
  /** Bumped after first-page load/search so RecordList can fill a short viewport. */
  const viewportFillToken = ref(0);

  // === Getters ===
  /** Reuse last merged object when id/list row/detail identity unchanged. */
  let selectedRecordCache: {
    id: number;
    base: ClipboardRecord;
    detail: ClipboardRecord | undefined;
    merged: ClipboardRecord;
  } | null = null;

  const selectedRecord = computed(() => {
    const base = records.value.find((r) => r.id === selectedId.value) ?? null;
    if (!base) {
      selectedRecordCache = null;
      return null;
    }
    const detail = recordDetails.value.get(base.id);
    const cached = selectedRecordCache;
    if (
      cached &&
      cached.id === base.id &&
      cached.base === base &&
      cached.detail === detail
    ) {
      return cached.merged;
    }
    const merged = detail
      ? {
          ...base,
          content: detail.content,
          content_html: detail.content_html,
          content_len: detail.content_len ?? base.content_len,
          media_abs: detail.media_abs ?? base.media_abs,
          thumb_abs: detail.thumb_abs ?? base.thumb_abs,
          width: detail.width ?? base.width,
          height: detail.height ?? base.height,
        }
      : base;
    selectedRecordCache = { id: base.id, base, detail, merged };
    return merged;
  });

  /** M-4: Server applies category/tag/trash/search filters; list is ready to render.
   * Direct ref alias (not a computed) — avoids an extra reactive caching layer
   * on every access. Name kept for API compatibility. */
  const filteredRecords = records;

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

  // === Scheduler (expiry sweep + stats debounce) ===
  const expiry = createExpiryScheduler({ records, selectedId, selectedIds, recordDetails, stats });
  const { scheduleLoadStats, purgeExpiredRecords, removeExpiredFromList, loadStats } = expiry;

  // === Trash count (shared by list + record actions) ===
  async function loadTrashCount() {
    try {
      trashCount.value = await invoke<number>("get_trash_count");
    } catch (e) {
      console.error("Load trash count failed:", e);
    }
  }

  // === List / pagination (late-binds tag reload to break the tag↔list cycle) ===
  const tagReload: { fn: () => void } = { fn: () => {} };
  const list = createListActions({
    records,
    selectedId,
    lastIncomingId,
    hasMore,
    isLoading,
    isLoadingMore,
    isSearching,
    searchQuery,
    activeFilter,
    activeTag,
    trashFilter,
    listSort,
    recordDetails,
    viewportFillToken,
    scheduleLoadStats,
    loadStats,
    loadTrashCount,
    scheduleLoadTags: () => tagReload.fn(),
  });
  const {
    loadRecords,
    loadMore,
    search,
    reloadList,
    setListSort,
    ensureRecordDetail,
    onNewRecord,
    reorderForUpdates,
    reorderForUpdate,
  } = list;

  function setPinnedCollapsed(collapsed: boolean) {
    if (pinnedCollapsed.value === collapsed) return;
    pinnedCollapsed.value = collapsed;
    persistPinnedCollapsed(collapsed);
  }

  function togglePinnedCollapsed() {
    setPinnedCollapsed(!pinnedCollapsed.value);
  }

  // === Record mutations ===
  const record = createRecordActions({
    records,
    selectedId,
    selectedIds,
    batchMode,
    recordDetails,
    trashCount,
    listSort,
    setPinnedCollapsed,
    patchRecord: list.patchRecord,
    patchRecordsBatch: list.patchRecordsBatch,
    reloadList,
    scheduleLoadStats,
    loadTrashCount,
    invalidateLoads: list.invalidateLoads,
    reorderForUpdates,
    scheduleLoadTags: () => tagReload.fn(),
  });
  const {
    aiBusyId,
    pasteRecord,
    deleteRecord,
    toggleFavorite,
    batchFavorite,
    togglePin,
    setAlias,
    enrichRecord,
    deleteBatch,
    restoreRecord,
    restoreRecordsBatch,
    permanentlyDeleteRecord,
    permanentlyDeleteRecordsBatch,
    emptyTrash,
  } = record;

  // === Tag Actions (extracted to clipboardTagActions.ts) ===
  const tagActions = createTagActions({
    tags,
    records,
    activeTag,
    activeFilter,
    trashFilter,
    selectedId,
    recordDetails,
    patchRecord: list.patchRecord,
    patchRecordsBatch: list.patchRecordsBatch,
    reloadList,
    reorderForUpdates,
  });
  tagReload.fn = tagActions.scheduleLoadTags;
  const {
    scheduleLoadTags,
    loadTags,
    createTag,
    deleteTag,
    updateTag,
    addTagToRecord,
    removeTagFromRecord,
    setRecordTags,
    filterByTag,
  } = tagActions;

  // === Selection / filter primitives ===
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

  function setFilter(filter: FilterTab) {
    activeFilter.value = filter;
    // Keep activeTag — type/favorites and tag combine with AND.
    selectedId.value = null;
    reloadList();
    scheduleLoadTags();
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
    scheduleLoadTags();
  }

  function toggleBatchMode() {
    if (!featureEnabled("batch")) {
      batchMode.value = false;
      selectedIds.value = new Set();
      return;
    }
    batchMode.value = !batchMode.value;
    if (!batchMode.value) {
      selectedIds.value = new Set();
      batchAnchorId.value = null;
    }
  }

  function toggleBatchSelect(id: number) {
    const next = new Set(selectedIds.value);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds.value = next;
    batchAnchorId.value = id;
  }

  /** Inclusive range from the last toggle/click through `id` (Shift+click). */
  function selectBatchRange(id: number) {
    if (!batchMode.value) return;
    const list = records.value;
    const anchor = batchAnchorId.value ?? id;
    const a = list.findIndex((r) => r.id === anchor);
    const b = list.findIndex((r) => r.id === id);
    if (a < 0 || b < 0) {
      toggleBatchSelect(id);
      return;
    }
    const [lo, hi] = a < b ? [a, b] : [b, a];
    const next = new Set(selectedIds.value);
    for (let i = lo; i <= hi; i++) next.add(list[i].id);
    selectedIds.value = next;
    batchAnchorId.value = id;
  }

  /** Select every currently loaded row (batch mode only). */
  function selectAllFiltered() {
    if (!batchMode.value) return;
    selectedIds.value = new Set(records.value.map((r) => r.id));
  }

  function clearBatchSelection() {
    selectedIds.value = new Set();
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
      // Silent failure here means the user believes capture is paused while
      // it keeps recording — always surface the error.
      useToast().toast(humanizeInvokeError(e, i18n.global.t), "error");
    }
  }

  async function importRecords(importedRecords: ClipboardRecord[]) {
    // Raw JSON string: the backend validates size/count before materializing
    // the Vec instead of deserializing an unbounded argument.
    const imported = await invoke<number>("import_data", {
      records_json: JSON.stringify(importedRecords),
    });
    await loadRecords();
    return imported;
  }

  // M-1: Watch source uses string comparison (RFC3339 is lexicographically
  // ordered) to avoid N Date parses on every records change. The callback
  // (scheduleExpireSweep) only runs when the signature actually changes.
  watch(
    () => {
      let count = 0;
      let nearest = "";
      for (const r of records.value) {
        if (!r.auto_expire_at) continue;
        count++;
        if (!nearest || r.auto_expire_at < nearest) nearest = r.auto_expire_at;
      }
      return count === 0 ? "0" : `${count}:${nearest}`;
    },
    () => {
      expiry.scheduleExpireSweep();
    }
  );

  // Re-enabling the tags feature (Settings → Features) must refresh the tag
  // list: while disabled, tags were cleared and nothing else triggers a
  // reload until the user happens to change a filter.
  const settingsStore = useSettingsStore();
  watch(
    () => settingsStore.settings.features?.tags ?? false,
    (enabled, wasEnabled) => {
      if (!enabled || wasEnabled) return;
      // The backend gates get_all_tags on its *persisted* settings, which lag
      // behind the reactive store (autosave is debounced). Persist first, then
      // reload, or the re-enabled request is rejected with "feature disabled".
      void settingsStore.saveSettings().finally(() => loadTags());
    },
  );

  return {
    // State
    records,
    lastIncomingId,
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
    listSort,
    pinnedCollapsed,
    batchMode,
    selectedIds,
    pauseCapture,
    stats,
    tags,
    viewportFillToken,
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
    setListSort,
    setPinnedCollapsed,
    togglePinnedCollapsed,
    reloadList,
    pasteRecord,
    deleteRecord,
    toggleFavorite,
    batchFavorite,
    togglePin,
    setAlias,
    enrichRecord,
    aiBusyId,
    deleteBatch,
    restoreRecord,
    restoreRecordsBatch,
    permanentlyDeleteRecord,
    permanentlyDeleteRecordsBatch,
    purgeExpiredRecords,
    removeExpiredFromList,
    emptyTrash,
    loadTrashCount,
    setTrashFilter,
    toggleBatchMode,
    toggleBatchSelect,
    selectBatchRange,
    selectAllFiltered,
    clearBatchSelection,
    setPauseCapture,
    togglePauseCapture,
    ensureRecordDetail,
    onNewRecord,
    reorderForUpdates,
    reorderForUpdate,
    loadStats,
    scheduleLoadStats,
    importRecords,
    loadTags,
    scheduleLoadTags,
    createTag,
    deleteTag,
    updateTag,
    setRecordTags,
    addTagToRecord,
    removeTagFromRecord,
    filterByTag,
  };
});
