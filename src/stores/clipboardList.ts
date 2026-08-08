/**
 * List / pagination / search store actions extracted from clipboard.ts to
 * reduce file size. The store public API is unchanged — methods are spread
 * back into the store return.
 *
 * `FilterTab`, `ListSort` and `LIST_SORT_OPTIONS` also live here (they are
 * list-domain) and are re-exported from `./clipboard` for existing importers.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type { ClipboardRecord, RecordsPage, SearchResult } from "../types";
import { featureEnabled } from "../composables/useFeature";

export type { FilterTab } from "../types";
import type { FilterTab } from "../types";
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

/** Merge a patch into a cached detail record (copy-on-write; no-op if absent). */
export function detailUpsert(
  recordDetails: Ref<Map<number, ClipboardRecord>>,
  id: number,
  patch: Partial<ClipboardRecord>,
) {
  const detail = recordDetails.value.get(id);
  if (!detail) return;
  const next = new Map(recordDetails.value);
  next.set(id, { ...detail, ...patch });
  recordDetails.value = next;
}

/** Drop detail records by id (copy-on-write; no-op when nothing matches). */
export function detailRemove(
  recordDetails: Ref<Map<number, ClipboardRecord>>,
  ids: number | number[],
) {
  const list = Array.isArray(ids) ? ids : [ids];
  const next = new Map(recordDetails.value);
  let changed = false;
  for (const id of list) {
    if (next.delete(id)) changed = true;
  }
  if (changed) recordDetails.value = next;
}

export interface ListActionsCtx {
  records: Ref<ClipboardRecord[]>;
  selectedId: Ref<number | null>;
  lastIncomingId: Ref<number | null>;
  hasMore: Ref<boolean>;
  isLoading: Ref<boolean>;
  isLoadingMore: Ref<boolean>;
  isSearching: Ref<boolean>;
  searchQuery: Ref<string>;
  activeFilter: Ref<FilterTab>;
  activeTag: Ref<string | null>;
  trashFilter: Ref<boolean>;
  listSort: Ref<ListSort>;
  recordDetails: Ref<Map<number, ClipboardRecord>>;
  viewportFillToken: Ref<number>;
  scheduleLoadStats: () => void;
  loadTrashCount: () => Promise<void>;
  /** Late-bound tag reload — breaks the list↔tags construction cycle. */
  scheduleLoadTags: () => void;
}

export function createListActions(ctx: ListActionsCtx) {
  const PAGE_SIZE = 60;
  const DETAIL_CACHE_MAX = 6;
  /** Byte budget for cached full bodies (content + HTML) — huge clips must
   * not balloon memory just because the row cap is small. */
  const DETAIL_CACHE_MAX_BYTES = 8 * 1024 * 1024;
  /** Soft cap for in-memory list (onNewRecord prepend without bound was a leak). */
  const LIST_SOFT_CAP = PAGE_SIZE * 2;
  let searchSeq = 0;
  let loadSeq = 0;
  /** Server-side offset for the next loadMore (not records.length — soft-cap may trim). */
  let listFetchOffset = 0;
  /** Soft-cap dropped rows → offset window has holes; next loadMore reloads. */
  let listWindowDirty = false;
  /** L-4: In-flight detail fetches — prevents duplicate IPC when selection changes rapidly. */
  const detailInFlight = new Set<number>();
  let incomingFlashTimer: ReturnType<typeof setTimeout> | null = null;
  let sortReloadTimer: ReturnType<typeof setTimeout> | null = null;

  /** Bumped after first-page load/search so RecordList can fill a short viewport (no isLoading watch). */
  function requestViewportFill() {
    ctx.viewportFillToken.value += 1;
  }

  function listQueryArgs(
    offset: number,
    cursor?: { before_pinned: number; before_updated_at: string; before_id: number } | null
  ) {
    const favoritesOnly = !ctx.trashFilter.value && ctx.activeFilter.value === "favorites";
    // Must match #[tauri::command(rename_all = "snake_case")] on get_records.
    return {
      limit: PAGE_SIZE,
      offset,
      trashed: ctx.trashFilter.value,
      content_type:
        !ctx.trashFilter.value && !favoritesOnly && ctx.activeFilter.value !== "all"
          ? ctx.activeFilter.value
          : null,
      favorites_only: favoritesOnly,
      tag: featureEnabled("tags") && !ctx.trashFilter.value ? ctx.activeTag.value : null,
      sort: ctx.listSort.value,
      before_pinned: cursor?.before_pinned ?? null,
      before_updated_at: cursor?.before_updated_at ?? null,
      before_id: cursor?.before_id ?? null,
    };
  }

  function searchFilterArgs(cursor?: {
    before_pinned: number
    before_updated_at: string
    before_id: number
  }) {
    const favoritesOnly = ctx.activeFilter.value === "favorites";
    // Must match #[tauri::command(rename_all = "snake_case")] on search_records.
    return {
      content_type:
        !favoritesOnly && ctx.activeFilter.value !== "all" ? ctx.activeFilter.value : null,
      favorites_only: favoritesOnly,
      tag: featureEnabled("tags") ? ctx.activeTag.value : null,
      sort: ctx.listSort.value,
      // Keyset cursor for the default newest-first sort (null → OFFSET page).
      before_pinned: cursor?.before_pinned ?? null,
      before_updated_at: cursor?.before_updated_at ?? null,
      before_id: cursor?.before_id ?? null,
    };
  }

  function reloadList() {
    if (ctx.searchQuery.value.trim()) {
      void search(ctx.searchQuery.value);
    } else {
      void loadRecords();
    }
  }

  function appendRecords(batch: ClipboardRecord[]) {
    // Build a persistent-style Set once per call; batch is small (PAGE_SIZE).
    const seen = new Set<number>();
    for (const r of ctx.records.value) seen.add(r.id);
    for (const r of batch) {
      if (!seen.has(r.id)) {
        ctx.records.value.push(r);
        seen.add(r.id);
      }
    }
    // NOTE: no soft-cap trim here — trimming paginated rows resets the keyset
    // cursor (list tail) and livelocks loadMore (same page re-fetched forever).
    // The cap still applies to onNewRecord prepends via applySoftCap.
  }

  async function loadRecords() {
    const seq = ++loadSeq;
    ctx.isLoading.value = true;
    ctx.isLoadingMore.value = false;
    ctx.hasMore.value = true;
    listWindowDirty = false;
    try {
      const page = await invoke<RecordsPage>("get_records", listQueryArgs(0));
      if (seq !== loadSeq) return;
      ctx.records.value = page.records;
      ctx.hasMore.value = page.has_more;
      listFetchOffset = page.records.length;
      ctx.recordDetails.value = new Map();
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
      // Preserve selection: re-fetch full detail after list truncated rows replaced cache.
      if (ctx.selectedId.value !== null) {
        void ensureRecordDetail(ctx.selectedId.value);
      }
      if (ctx.hasMore.value) requestViewportFill();
    } catch (e) {
      console.error("Failed to load records:", e);
    } finally {
      if (seq === loadSeq) ctx.isLoading.value = false;
    }
  }

  async function loadMore() {
    if (!ctx.hasMore.value || ctx.isLoading.value || ctx.isLoadingMore.value) return;
    // Offset-based sorts (created_desc, copies_desc, updated_asc) can skip rows
    // when the soft cap has trimmed the local window; keyset pagination for
    // updated_desc is stable against prepends/trim so we continue normally.
    if (listWindowDirty && ctx.listSort.value !== "updated_desc") {
      await reloadList();
      return;
    }
    const seq = loadSeq;
    ctx.isLoadingMore.value = true;
    try {
      if (ctx.searchQuery.value.trim()) {
        if (ctx.listSort.value === "updated_desc" && ctx.records.value.length > 0) {
          // Keyset cursor — search results are stable against new captures that
          // match the query (no OFFSET drift across pages).
          const last = ctx.records.value[ctx.records.value.length - 1];
          const result = await invoke<SearchResult>("search_records", {
            query: ctx.searchQuery.value,
            limit: PAGE_SIZE,
            offset: 0,
            ...searchFilterArgs({
              before_pinned: last.is_pinned ? 1 : 0,
              before_updated_at: last.updated_at,
              before_id: last.id,
            }),
          });
          if (seq !== loadSeq || ctx.trashFilter.value) return;
          appendRecords(result.records);
          listFetchOffset = 0;
          ctx.hasMore.value = result.has_more;
        } else {
          const offset = listFetchOffset;
          const result = await invoke<SearchResult>("search_records", {
            query: ctx.searchQuery.value,
            limit: PAGE_SIZE,
            offset,
            ...searchFilterArgs(),
          });
          if (seq !== loadSeq || ctx.trashFilter.value) return;
          appendRecords(result.records);
          listFetchOffset = offset + result.records.length;
          ctx.hasMore.value = result.has_more;
        }
      } else if (ctx.listSort.value === "updated_desc" && ctx.records.value.length > 0) {
        // Keyset cursor — stable when new rows are prepended during scroll.
        const last = ctx.records.value[ctx.records.value.length - 1];
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
        ctx.hasMore.value = page.has_more;
      } else {
        const offset = listFetchOffset;
        const page = await invoke<RecordsPage>("get_records", listQueryArgs(offset));
        if (seq !== loadSeq) return;
        appendRecords(page.records);
        listFetchOffset = offset + page.records.length;
        ctx.hasMore.value = page.has_more;
      }
    } catch (e) {
      console.error("Failed to load more records:", e);
    } finally {
      if (seq === loadSeq) ctx.isLoadingMore.value = false;
    }
  }

  async function search(query: string) {
    if (!query.trim()) {
      ctx.searchQuery.value = "";
      ctx.isSearching.value = false;
      await loadRecords();
      return;
    }
    const capturedSeq = ++searchSeq;
    ++loadSeq;
    const seq = loadSeq;
    ctx.isSearching.value = true;
    ctx.isLoading.value = true;
    ctx.searchQuery.value = query;
    ctx.hasMore.value = true;
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
      if (capturedSeq !== searchSeq || ctx.trashFilter.value || seq !== loadSeq) {
        return;
      }
      ctx.records.value = result.records;
      ctx.hasMore.value = result.has_more;
      listFetchOffset = result.records.length;
      ctx.recordDetails.value = new Map();
      if (ctx.selectedId.value !== null) {
        void ensureRecordDetail(ctx.selectedId.value);
      }
      if (ctx.hasMore.value) requestViewportFill();
    } catch (e) {
      console.error("Search failed:", e);
    } finally {
      if (capturedSeq === searchSeq) {
        ctx.isSearching.value = false;
        ctx.isLoading.value = false;
      }
    }
  }

  function pruneRecordDetails(keepId?: number | null) {
    const alive = new Set(ctx.records.value.map((r) => r.id));
    if (keepId != null) alive.add(keepId);
    const next = new Map<number, ClipboardRecord>();
    for (const [id, detail] of ctx.recordDetails.value) {
      if (alive.has(id)) next.set(id, detail);
    }
    const bytesOf = (d: ClipboardRecord) =>
      (d.content_len ?? d.content.length) + (d.content_html?.length ?? 0);
    // Cap cache size (LRU-ish: prefer selected, then newest inserts order)
    if (next.size > DETAIL_CACHE_MAX) {
      const ids = [...next.keys()].filter((id) => id !== keepId && id !== ctx.selectedId.value);
      for (const id of ids) {
        if (next.size <= DETAIL_CACHE_MAX) break;
        next.delete(id);
      }
    }
    // Byte budget: evict non-selected rows until the cache fits the budget.
    let total = 0;
    for (const d of next.values()) total += bytesOf(d);
    if (total > DETAIL_CACHE_MAX_BYTES) {
      const ids = [...next.keys()].filter((id) => id !== keepId && id !== ctx.selectedId.value);
      for (const id of ids) {
        if (total <= DETAIL_CACHE_MAX_BYTES) break;
        const d = next.get(id);
        if (!d) continue;
        total -= bytesOf(d);
        next.delete(id);
      }
    }
    ctx.recordDetails.value = next;
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
    ctx.hasMore.value = true;
    if (ctx.selectedId.value !== null && !next.some((r) => r.id === ctx.selectedId.value)) {
      ctx.selectedId.value = null;
    }
    pruneRecordDetails(ctx.selectedId.value);
    return next;
  }

  /**
   * Replace (not mutate) a list row. selectedRecord caches its merged view by
   * object-reference equality, so in-place mutation (record.x = v) leaves the
   * cached snapshot stale and PreviewPane never reflects the change. Swapping
   * in a new object invalidates that cache and re-renders dependents.
   */
  function patchRecord(id: number, patch: Partial<ClipboardRecord>) {
    const idx = ctx.records.value.findIndex((r) => r.id === id);
    if (idx === -1) return;
    const next = ctx.records.value.slice();
    next[idx] = { ...next[idx], ...patch };
    ctx.records.value = next;
  }

  /**
   * Batch-patch multiple records in ONE array copy + ONE reactive trigger.
   * Eliminates O(N²) when patchRecord is called in a loop (deleteTag, updateTag).
   */
  function patchRecordsBatch(patches: Map<number, Partial<ClipboardRecord>>) {
    if (patches.size === 0) return;
    const next = ctx.records.value.slice();
    let changed = false;
    for (let i = 0; i < next.length; i++) {
      const patch = patches.get(next[i].id);
      if (patch) {
        next[i] = { ...next[i], ...patch };
        changed = true;
      }
    }
    if (changed) ctx.records.value = next;
  }

  /** Lazy-load full content / HTML into a separate detail cache (not list rows). */
  async function ensureRecordDetail(id: number) {
    if (ctx.recordDetails.value.has(id)) return;
    if (detailInFlight.has(id)) return; // L-4: dedup concurrent calls
    const record = ctx.records.value.find((r) => r.id === id);
    if (!record || record.content_type === "image") return;
    detailInFlight.add(id);
    try {
      const full = await invoke<ClipboardRecord | null>("get_record", { id });
      if (!full || ctx.selectedId.value !== id) return;
      const next = new Map(ctx.recordDetails.value);
      next.set(id, full);
      ctx.recordDetails.value = next;
      pruneRecordDetails(id);
    } catch (e) {
      console.error("Failed to load record detail:", e);
    } finally {
      detailInFlight.delete(id);
    }
  }

  /** Mark a record id briefly so the list can flash its row (capture feedback). */
  function flashIncoming(id: number) {
    ctx.lastIncomingId.value = id;
    if (incomingFlashTimer) clearTimeout(incomingFlashTimer);
    incomingFlashTimer = setTimeout(() => {
      incomingFlashTimer = null;
      ctx.lastIncomingId.value = null;
    }, 1000);
  }

  function scheduleReloadList() {
    if (sortReloadTimer) clearTimeout(sortReloadTimer);
    sortReloadTimer = setTimeout(() => {
      sortReloadTimer = null;
      reloadList();
    }, 400);
  }

  /** Called by event listener when clipboard changes. */
  function onNewRecord(record: ClipboardRecord) {
    ctx.scheduleLoadStats();
    if (record.tags.length > 0) {
      ctx.scheduleLoadTags();
    }
    if (ctx.trashFilter.value || ctx.searchQuery.value) return;
    if (ctx.activeTag.value && !record.tags.includes(ctx.activeTag.value)) return;
    if (ctx.activeFilter.value === "favorites" && !record.is_favorite) return;
    if (
      ctx.activeFilter.value !== "all" &&
      ctx.activeFilter.value !== "favorites" &&
      record.content_type !== ctx.activeFilter.value
    ) {
      return;
    }
    // Only the default "newest first" order can safely prepend; other sorts reload (debounced).
    if (ctx.listSort.value !== "updated_desc") {
      scheduleReloadList();
      return;
    }
    const list = ctx.records.value;
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
    // Pinned records go to the top of the pinned section (newest first);
    // unpinned ones land right after the pinned block.
    const insertAt = record.is_pinned ? 0 : pinCount;
    next.splice(insertAt, 0, record);
    ctx.records.value = applySoftCap(next);
    flashIncoming(record.id);
  }

  function setListSort(sort: ListSort) {
    if (ctx.listSort.value === sort) return;
    ctx.listSort.value = sort;
    reloadList();
  }

  /** Invalidate in-flight list loads (e.g. after emptyTrash clears the list). */
  function invalidateLoads() {
    loadSeq += 1;
    // The visible window was replaced out-of-band: reset server-side pagination
    // too, otherwise a stale offset/hasMore yields mis-paged or redundant
    // loadMore requests on the next scroll.
    listFetchOffset = 0;
    listWindowDirty = false;
    ctx.hasMore.value = false;
  }

  return {
    loadRecords,
    loadMore,
    search,
    reloadList,
    setListSort,
    ensureRecordDetail,
    onNewRecord,
    patchRecord,
    patchRecordsBatch,
    invalidateLoads,
  };
}
