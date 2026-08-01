import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRecord, RecordsPage, SearchResult, StatsData, Tag } from "../types";
import { setPasteFocusLock } from "../composables/pasteFocusLock";
import { createTagActions } from "./clipboardTagActions";

export type FilterTab = 'all' | 'text' | 'code' | 'link' | 'image' | 'file' | 'favorites';
export type ListSort =
  | "updated_desc"
  | "updated_asc"
  | "created_desc"
  | "copies_desc";

export const LIST_SORT_OPTIONS: { value: ListSort; labelKey: string }[] = [
  { value: "updated_desc", labelKey: "sort.updatedDesc" },
  { value: "updated_asc", labelKey: "sort.updatedAsc" },
  { value: "created_desc", labelKey: "sort.createdDesc" },
  { value: "copies_desc", labelKey: "sort.copiesDesc" },
];

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
  const listSort = ref<ListSort>("updated_desc");
  const batchMode = ref(false);
  const selectedIds = ref<Set<number>>(new Set());
  const pauseCapture = ref(false);
  const stats = ref<StatsData | null>(null);
  const tags = ref<Tag[]>([]);
  /** Full content/HTML for preview only — never merge back into list rows. */
  const recordDetails = ref<Map<number, ClipboardRecord>>(new Map());
  /** Freshly captured record id — drives the row-flash highlight in the list. */
  const lastIncomingId = ref<number | null>(null);
  let incomingFlashTimer: ReturnType<typeof setTimeout> | null = null;
  let searchSeq = 0;
  let loadSeq = 0;
  let expireSweepTimer: ReturnType<typeof setTimeout> | null = null;
  let expireSweepRunning = false;


  /** Soft cap for in-memory list (onNewRecord prepend without bound was a leak). */
  const LIST_SOFT_CAP = PAGE_SIZE * 2;
  /** Server-side offset for the next loadMore (not records.length — soft-cap may trim). */
  let listFetchOffset = 0;
  /** Soft-cap dropped rows → offset window has holes; next loadMore reloads. */
  let listWindowDirty = false;
  /** Bumped after first-page load/search so RecordList can fill a short viewport (no isLoading watch). */
  const viewportFillToken = ref(0);

  function requestViewportFill() {
    viewportFillToken.value += 1;
  }
  const DETAIL_CACHE_MAX = 6;

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

  function listQueryArgs(
    offset: number,
    cursor?: { before_pinned: number; before_updated_at: string; before_id: number } | null
  ) {
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
      sort: listSort.value,
      before_pinned: cursor?.before_pinned ?? null,
      before_updated_at: cursor?.before_updated_at ?? null,
      before_id: cursor?.before_id ?? null,
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
      sort: listSort.value,
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
    // Build a persistent-style Set once per call; batch is small (PAGE_SIZE).
    const seen = new Set<number>();
    for (const r of records.value) seen.add(r.id);
    for (const r of batch) {
      if (!seen.has(r.id)) {
        records.value.push(r);
        seen.add(r.id);
      }
    }
    // NOTE: no soft-cap trim here — trimming paginated rows resets the keyset
    // cursor (list tail) and livelocks loadMore (same page re-fetched forever).
    // The cap still applies to onNewRecord prepends via applySoftCap.
  }

  // === Actions ===
  async function loadRecords() {
    const seq = ++loadSeq;
    isLoading.value = true;
    isLoadingMore.value = false;
    hasMore.value = true;
    listWindowDirty = false;
    try {
      const page = await invoke<RecordsPage>("get_records", listQueryArgs(0));
      if (seq !== loadSeq) return;
      records.value = page.records;
      hasMore.value = page.has_more;
      listFetchOffset = page.records.length;
      recordDetails.value = new Map();
      scheduleLoadStats();
      await loadTrashCount();
      // Preserve selection: re-fetch full detail after list truncated rows replaced cache.
      if (selectedId.value !== null) {
        void ensureRecordDetail(selectedId.value);
      }
      if (hasMore.value) requestViewportFill();
    } catch (e) {
      console.error("Failed to load records:", e);
    } finally {
      if (seq === loadSeq) isLoading.value = false;
    }
  }

  async function loadMore() {
    if (!hasMore.value || isLoading.value || isLoadingMore.value) return;
    // Offset-based sorts (created_desc, copies_desc, updated_asc) can skip rows
    // when the soft cap has trimmed the local window; keyset pagination for
    // updated_desc is stable against prepends/trim so we continue normally.
    if (listWindowDirty && listSort.value !== "updated_desc") {
      await reloadList();
      return;
    }
    const seq = loadSeq;
    isLoadingMore.value = true;
    try {
      if (searchQuery.value.trim()) {
        const offset = listFetchOffset;
        const result = await invoke<SearchResult>("search_records", {
          query: searchQuery.value,
          limit: PAGE_SIZE,
          offset,
          ...searchFilterArgs(),
        });
        if (seq !== loadSeq || trashFilter.value) return;
        appendRecords(result.records);
        listFetchOffset = offset + result.records.length;
        hasMore.value = result.has_more;
      } else if (listSort.value === "updated_desc" && records.value.length > 0) {
        // Keyset cursor — stable when new rows are prepended during scroll.
        const last = records.value[records.value.length - 1];
        const page = await invoke<RecordsPage>(
          "get_records",
          listQueryArgs(0, {
            before_pinned: last.is_pinned ? 1 : 0,
            before_updated_at: last.updated_at,
            before_id: last.id,
          })
        );
        if (seq !== loadSeq) return;
        appendRecords(page.records);
        hasMore.value = page.has_more;
      } else {
        const offset = listFetchOffset;
        const page = await invoke<RecordsPage>("get_records", listQueryArgs(offset));
        if (seq !== loadSeq) return;
        appendRecords(page.records);
        listFetchOffset = offset + page.records.length;
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
    listWindowDirty = false;
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
      listFetchOffset = result.records.length;
      recordDetails.value = new Map();
      if (selectedId.value !== null) {
        void ensureRecordDetail(selectedId.value);
      }
      if (hasMore.value) requestViewportFill();
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

  function pruneRecordDetails(keepId?: number | null) {
    const alive = new Set(records.value.map((r) => r.id));
    if (keepId != null) alive.add(keepId);
    const next = new Map<number, ClipboardRecord>();
    for (const [id, detail] of recordDetails.value) {
      if (alive.has(id)) next.set(id, detail);
    }
    // Cap cache size (LRU-ish: prefer selected, then newest inserts order)
    if (next.size > DETAIL_CACHE_MAX) {
      const ids = [...next.keys()].filter((id) => id !== keepId && id !== selectedId.value);
      for (const id of ids) {
        if (next.size <= DETAIL_CACHE_MAX) break;
        next.delete(id);
      }
    }
    recordDetails.value = next;
  }

  function applySoftCap(list: ClipboardRecord[]): ClipboardRecord[] {
    if (list.length <= LIST_SOFT_CAP) return list;
    const pinned: ClipboardRecord[] = [];
    const rest: ClipboardRecord[] = [];
    for (const r of list) {
      if (r.is_pinned) pinned.push(r);
      else rest.push(r);
    }
    const restKeep = Math.max(0, LIST_SOFT_CAP - pinned.length);
    const next = pinned.concat(rest.slice(0, restKeep));
    // Local window no longer matches contiguous server offsets.
    listWindowDirty = true;
    hasMore.value = true;
    if (selectedId.value !== null && !next.some((r) => r.id === selectedId.value)) {
      selectedId.value = null;
    }
    pruneRecordDetails(selectedId.value);
    return next;
  }

  /**
   * Replace (not mutate) a list row. selectedRecord caches its merged view by
   * object-reference equality, so in-place mutation (record.x = v) leaves the
   * cached snapshot stale and PreviewPane never reflects the change. Swapping
   * in a new object invalidates that cache and re-renders dependents.
   */
  function patchRecord(id: number, patch: Partial<ClipboardRecord>) {
    const idx = records.value.findIndex((r) => r.id === id);
    if (idx === -1) return;
    const next = records.value.slice();
    next[idx] = { ...next[idx], ...patch };
    records.value = next;
  }

  /**
   * Batch-patch multiple records in ONE array copy + ONE reactive trigger.
   * Eliminates O(N²) when patchRecord is called in a loop (deleteTag, updateTag).
   */
  function patchRecordsBatch(patches: Map<number, Partial<ClipboardRecord>>) {
    if (patches.size === 0) return;
    const next = records.value.slice();
    let changed = false;
    for (let i = 0; i < next.length; i++) {
      const patch = patches.get(next[i].id);
      if (patch) {
        next[i] = { ...next[i], ...patch };
        changed = true;
      }
    }
    if (changed) records.value = next;
  }

  /** L-4: In-flight detail fetches — prevents duplicate IPC when selection changes rapidly. */
  const detailInFlight = new Set<number>();

  /** Lazy-load full content / HTML into a separate detail cache (not list rows). */
  async function ensureRecordDetail(id: number) {
    if (recordDetails.value.has(id)) return;
    if (detailInFlight.has(id)) return; // L-4: dedup concurrent calls
    const record = records.value.find((r) => r.id === id);
    if (!record || record.content_type === "image") return;
    detailInFlight.add(id);
    try {
      const full = await invoke<ClipboardRecord | null>("get_record", { id });
      if (!full || selectedId.value !== id) return;
      const next = new Map(recordDetails.value);
      next.set(id, full);
      recordDetails.value = next;
      pruneRecordDetails(id);
    } catch (e) {
      console.error("Failed to load record detail:", e);
    } finally {
      detailInFlight.delete(id);
    }
  }

  function setFilter(filter: FilterTab) {
    activeFilter.value = filter;
    // Keep activeTag — type/favorites and tag combine with AND.
    selectedId.value = null;
    reloadList();
    scheduleLoadTags();
  }

  async function pasteRecord(id: number, mode: "original" | "plain" = "original") {
    setPasteFocusLock(true);
    try {
      await invoke("paste_record", { id, mode });
      const row = records.value.find((r) => r.id === id);
      if (row) patchRecord(id, { copy_count: row.copy_count + 1 });
      const detail = recordDetails.value.get(id);
      if (detail) {
        const next = new Map(recordDetails.value);
        next.set(id, { ...detail, copy_count: detail.copy_count + 1 });
        recordDetails.value = next;
      }
    } catch (e) {
      console.error("Paste failed:", e);
      throw e;
    } finally {
      setPasteFocusLock(false);
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
      const patches = new Map<number, Partial<ClipboardRecord>>();
      for (const id of toFav) patches.set(id, { is_favorite: true });
      patchRecordsBatch(patches);
      scheduleLoadStats();
    } catch (e) {
      console.error("Batch favorite failed:", e);
      throw e;
    }
  }

  async function deleteRecord(id: number) {
    try {
      await invoke("delete_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (recordDetails.value.has(id)) {
        const next = new Map(recordDetails.value);
        next.delete(id);
        recordDetails.value = next;
      }
      if (selectedId.value === id) selectedId.value = null;
      scheduleLoadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Delete failed:", e);
      throw e;
    }
  }

  async function toggleFavorite(id: number): Promise<boolean | null> {
    const record = records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { id });
      patchRecord(id, { is_favorite: newVal });
      scheduleLoadStats();
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
      patchRecord(id, { is_pinned: newVal });
      if (listSort.value === "updated_desc") {
        // Re-sort: pinned first, then by updated_at desc.
        // Use string comparison on ISO timestamps (lexicographic = chronological).
        records.value.sort((a, b) => {
          if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
          return b.updated_at.localeCompare(a.updated_at);
        });
      } else {
        reloadList();
      }
      scheduleLoadStats();
      return newVal;
    } catch (e) {
      console.error("Toggle pin failed:", e);
      return null;
    }
  }

  async function setAlias(id: number, alias: string): Promise<string | null> {
    const record = records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const saved = await invoke<string>("set_record_alias", { id, alias });
      patchRecord(id, { alias: saved });
      return saved;
    } catch (e) {
      console.error("Set alias failed:", e);
      return null;
    }
  }

  async function deleteBatch(ids: number[]) {
    try {
      await invoke("delete_records_batch", { ids });
      const idSet = new Set(ids);
      records.value = records.value.filter((r) => !idSet.has(r.id));
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value = new Set();
      batchMode.value = false;
      scheduleLoadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Batch delete failed:", e);
      throw e;
    }
  }

  // === Trash / Restore ===

  async function restoreRecord(id: number) {
    try {
      await invoke("restore_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (selectedId.value === id) selectedId.value = null;
      scheduleLoadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Restore failed:", e);
      throw e;
    }
  }

  async function restoreRecordsBatch(ids: number[]) {
    try {
      await invoke("restore_records_batch", { ids });
      const idSet = new Set(ids);
      records.value = records.value.filter((r) => !idSet.has(r.id));
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value = new Set();
      batchMode.value = false;
      scheduleLoadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Batch restore failed:", e);
      throw e;
    }
  }

  async function permanentlyDeleteRecord(id: number) {
    try {
      await invoke("permanently_delete_record", { id });
      records.value = records.value.filter((r) => r.id !== id);
      if (recordDetails.value.has(id)) {
        const next = new Map(recordDetails.value);
        next.delete(id);
        recordDetails.value = next;
      }
      if (selectedId.value === id) selectedId.value = null;
      await loadTrashCount();
    } catch (e) {
      console.error("Permanent delete failed:", e);
      throw e;
    }
  }

  async function permanentlyDeleteRecordsBatch(ids: number[]) {
    try {
      await invoke("permanently_delete_records_batch", { ids });
      const idSet = new Set(ids);
      records.value = records.value.filter((r) => !idSet.has(r.id));
      for (const id of ids) {
        if (recordDetails.value.has(id)) {
          const next = new Map(recordDetails.value);
          next.delete(id);
          recordDetails.value = next;
        }
      }
      if (selectedId.value !== null && selectedIds.value.has(selectedId.value)) {
        selectedId.value = null;
      }
      selectedIds.value = new Set();
      batchMode.value = false;
      await loadTrashCount();
    } catch (e) {
      console.error("Batch permanent delete failed:", e);
      throw e;
    }
  }

  /** Remove expired sensitive records from DB + local list; reschedule next sweep. */
  function removeExpiredFromList(ids: number[]) {
    if (ids.length === 0) return;
    const idSet = new Set(ids);
    records.value = records.value.filter((r) => !idSet.has(r.id));
    if (selectedId.value !== null && idSet.has(selectedId.value)) {
      selectedId.value = null;
    }
    if (selectedIds.value.size > 0) {
      const next = new Set([...selectedIds.value].filter((id) => !idSet.has(id)));
      selectedIds.value = next;
    }
    if (recordDetails.value.size > 0) {
      const details = new Map(recordDetails.value);
      for (const id of idSet) details.delete(id);
      recordDetails.value = details;
    }
  }

  async function purgeExpiredRecords() {
    if (expireSweepRunning) return;
    expireSweepRunning = true;
    try {
      const ids = await invoke<number[]>("cleanup_expired");
      removeExpiredFromList(ids);
      // Also drop any locally past-due rows (clock skew / missed event)
      const now = Date.now();
      const stale = records.value
        .filter((r) => r.auto_expire_at && new Date(r.auto_expire_at).getTime() <= now)
        .map((r) => r.id);
      if (stale.length > 0) {
        removeExpiredFromList(stale);
      }
      if (ids.length > 0 || stale.length > 0) {
        scheduleLoadStats();
      }
    } catch (e) {
      console.error("Purge expired failed:", e);
    } finally {
      expireSweepRunning = false;
      scheduleExpireSweep();
    }
  }

  function scheduleExpireSweep() {
    if (expireSweepTimer) {
      clearTimeout(expireSweepTimer);
      expireSweepTimer = null;
    }
    const now = Date.now();
    let nextAt = Infinity;
    for (const r of records.value) {
      if (!r.auto_expire_at) continue;
      // M-1: Only parse the nearest timestamp (one Date parse vs N).
      const t = new Date(r.auto_expire_at).getTime();
      if (Number.isNaN(t)) continue;
      if (t <= now) {
        void purgeExpiredRecords();
        return;
      }
      if (t < nextAt) nextAt = t;
    }
    if (nextAt < Infinity) {
      const delay = Math.max(50, nextAt - Date.now() + 30);
      expireSweepTimer = setTimeout(() => {
        expireSweepTimer = null;
        void purgeExpiredRecords();
      }, delay);
    }
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
      scheduleExpireSweep();
    }
  );

  let statsDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let statsMaxWaitTimer: ReturnType<typeof setTimeout> | null = null;

  /** Debounce 800ms while idle; max-wait 5s so continuous copy still refreshes stats. */
  function scheduleLoadStats() {
    if (statsDebounceTimer) clearTimeout(statsDebounceTimer);
    statsDebounceTimer = setTimeout(() => {
      statsDebounceTimer = null;
      if (statsMaxWaitTimer) {
        clearTimeout(statsMaxWaitTimer);
        statsMaxWaitTimer = null;
      }
      void loadStats();
    }, 800);

    if (!statsMaxWaitTimer) {
      statsMaxWaitTimer = setTimeout(() => {
        statsMaxWaitTimer = null;
        if (statsDebounceTimer) {
          clearTimeout(statsDebounceTimer);
          statsDebounceTimer = null;
        }
        void loadStats();
      }, 5000);
    }
  }

  async function emptyTrash() {
    try {
      await invoke("empty_trash");
      loadSeq += 1; // invalidate in-flight list loads
      records.value = [];
      selectedId.value = null;
      selectedIds.value = new Set();
      batchMode.value = false;
      trashCount.value = 0;
      recordDetails.value = new Map();
      scheduleLoadStats();
      await loadTrashCount();
    } catch (e) {
      console.error("Empty trash failed:", e);
      throw e;
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
    scheduleLoadTags();
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

  let sortReloadTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleReloadList() {
    if (sortReloadTimer) clearTimeout(sortReloadTimer);
    sortReloadTimer = setTimeout(() => {
      sortReloadTimer = null;
      reloadList();
    }, 400);
  }

  // Called by event listener when clipboard changes
  /** Mark a record id briefly so the list can flash its row (capture feedback). */
  function flashIncoming(id: number) {
    lastIncomingId.value = id;
    if (incomingFlashTimer) clearTimeout(incomingFlashTimer);
    incomingFlashTimer = setTimeout(() => {
      incomingFlashTimer = null;
      lastIncomingId.value = null;
    }, 1000);
  }

  function onNewRecord(record: ClipboardRecord) {
    scheduleLoadStats();
    if (record.tags.length > 0) {
      scheduleLoadTags();
    }
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
    // Only the default "newest first" order can safely prepend; other sorts reload (debounced).
    if (listSort.value !== "updated_desc") {
      scheduleReloadList();
      return;
    }
    const list = records.value;
    let pinCount = 0;
    let existingIdx = -1;
    for (let i = 0; i < list.length; i++) {
      const r = list[i];
      if (r.id === record.id) {
        existingIdx = i;
        continue;
      }
      if (r.is_pinned) pinCount += 1;
    }
    const next = list.slice();
    if (existingIdx !== -1) next.splice(existingIdx, 1);
    next.splice(pinCount, 0, record);
    records.value = applySoftCap(next);
    flashIncoming(record.id);
  }

  function setListSort(sort: ListSort) {
    if (listSort.value === sort) return;
    listSort.value = sort;
    reloadList();
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

  // === Tag Actions (extracted to clipboardTagActions.ts) ===
  const tagActions = createTagActions({
    tags,
    records,
    activeTag,
    activeFilter,
    trashFilter,
    selectedId,
    recordDetails,
    patchRecord,
    patchRecordsBatch,
    reloadList,
  });
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
    reloadList,
    pasteRecord,
    deleteRecord,
    toggleFavorite,
    batchFavorite,
    togglePin,
    setAlias,
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
    setPauseCapture,
    togglePauseCapture,
    ensureRecordDetail,
    onNewRecord,
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
