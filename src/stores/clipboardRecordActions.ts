/**
 * Record-mutation store actions (paste / favorite / pin / alias / batch /
 * trash) extracted from clipboard.ts to reduce file size. The store public
 * API is unchanged — methods are spread back into the store return.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type { ClipboardRecord } from "../types";
import { setPasteFocusLock } from "../composables/pasteFocusLock";
import type { ListSort } from "./clipboardList";
import { detailRemove, detailUpsert } from "./clipboardList";

export interface RecordActionsCtx {
  records: Ref<ClipboardRecord[]>;
  selectedId: Ref<number | null>;
  selectedIds: Ref<Set<number>>;
  batchMode: Ref<boolean>;
  recordDetails: Ref<Map<number, ClipboardRecord>>;
  trashCount: Ref<number>;
  listSort: Ref<ListSort>;
  patchRecord: (id: number, patch: Partial<ClipboardRecord>) => void;
  patchRecordsBatch: (patches: Map<number, Partial<ClipboardRecord>>) => void;
  reloadList: () => void;
  scheduleLoadStats: () => void;
  loadTrashCount: () => Promise<void>;
  invalidateLoads: () => void;
  reorderForUpdates: (ids: number[]) => void;
}

export function createRecordActions(ctx: RecordActionsCtx) {
  /** Returns whether Ctrl+V was sent (`false` = clipboard written, keys skipped). */
  async function pasteRecord(id: number, mode: "original" | "plain" = "original"): Promise<boolean> {
    setPasteFocusLock(true);
    try {
      const injected = await invoke<boolean>("paste_record", { id, mode });
      const row = ctx.records.value.find((r) => r.id === id);
      if (row) ctx.patchRecord(id, { copy_count: row.copy_count + 1 });
      const detail = ctx.recordDetails.value.get(id);
      if (detail) {
        detailUpsert(ctx.recordDetails, id, { copy_count: detail.copy_count + 1 });
      }
      return Boolean(injected);
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
      const r = ctx.records.value.find((x) => x.id === id);
      return r && !r.is_favorite;
    });
    if (!toFav.length) return;
    try {
      await invoke("batch_set_favorite", { ids: toFav, favorite: true });
      const patches = new Map<number, Partial<ClipboardRecord>>();
      for (const id of toFav) patches.set(id, { is_favorite: true });
      ctx.patchRecordsBatch(patches);
      ctx.scheduleLoadStats();
    } catch (e) {
      console.error("Batch favorite failed:", e);
      throw e;
    }
  }

  async function deleteRecord(id: number) {
    try {
      await invoke("delete_record", { id });
      ctx.records.value = ctx.records.value.filter((r) => r.id !== id);
      detailRemove(ctx.recordDetails, id);
      if (ctx.selectedId.value === id) ctx.selectedId.value = null;
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Delete failed:", e);
      throw e;
    }
  }

  async function toggleFavorite(id: number): Promise<boolean | null> {
    const record = ctx.records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { id });
      ctx.patchRecord(id, { is_favorite: newVal });
      ctx.scheduleLoadStats();
      return newVal;
    } catch (e) {
      console.error("Toggle favorite failed:", e);
      return null;
    }
  }

  async function togglePin(id: number): Promise<boolean | null> {
    const record = ctx.records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const newVal = await invoke<boolean>("toggle_pin", { id });
      ctx.patchRecord(id, { is_pinned: newVal });
      if (ctx.listSort.value === "updated_desc") {
        // Re-sort: pinned first, then by updated_at desc.
        // Use string comparison on ISO timestamps (lexicographic = chronological).
        // Assign a fresh array — never mutate the reactive list in place, or the
        // change bypasses reactivity and can drift from the keyset pagination state.
        ctx.records.value = [...ctx.records.value].sort((a, b) => {
          if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
          return b.updated_at.localeCompare(a.updated_at);
        });
      } else {
        ctx.reloadList();
      }
      ctx.scheduleLoadStats();
      return newVal;
    } catch (e) {
      console.error("Toggle pin failed:", e);
      return null;
    }
  }

  async function setAlias(id: number, alias: string): Promise<string | null> {
    const record = ctx.records.value.find((r) => r.id === id);
    if (!record) return null;
    try {
      const saved = await invoke<string>("set_record_alias", { id, alias });
      ctx.patchRecord(id, { alias: saved });
      // Rust bumps updated_at (sync watermark) only when the alias changed;
      // re-rank so the visible list mirrors the DB order like tag edits.
      if (saved !== record.alias) ctx.reorderForUpdates([id]);
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
      ctx.records.value = ctx.records.value.filter((r) => !idSet.has(r.id));
      if (ctx.selectedId.value !== null && ctx.selectedIds.value.has(ctx.selectedId.value)) {
        ctx.selectedId.value = null;
      }
      ctx.selectedIds.value = new Set();
      ctx.batchMode.value = false;
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Batch delete failed:", e);
      throw e;
    }
  }

  async function restoreRecord(id: number) {
    try {
      await invoke("restore_record", { id });
      ctx.records.value = ctx.records.value.filter((r) => r.id !== id);
      if (ctx.selectedId.value === id) ctx.selectedId.value = null;
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Restore failed:", e);
      throw e;
    }
  }

  async function restoreRecordsBatch(ids: number[]) {
    try {
      await invoke("restore_records_batch", { ids });
      const idSet = new Set(ids);
      ctx.records.value = ctx.records.value.filter((r) => !idSet.has(r.id));
      if (ctx.selectedId.value !== null && ctx.selectedIds.value.has(ctx.selectedId.value)) {
        ctx.selectedId.value = null;
      }
      ctx.selectedIds.value = new Set();
      ctx.batchMode.value = false;
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Batch restore failed:", e);
      throw e;
    }
  }

  async function permanentlyDeleteRecord(id: number) {
    try {
      await invoke("permanently_delete_record", { id });
      ctx.records.value = ctx.records.value.filter((r) => r.id !== id);
      detailRemove(ctx.recordDetails, id);
      if (ctx.selectedId.value === id) ctx.selectedId.value = null;
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Permanent delete failed:", e);
      throw e;
    }
  }

  async function permanentlyDeleteRecordsBatch(ids: number[]) {
    try {
      await invoke("permanently_delete_records_batch", { ids });
      const idSet = new Set(ids);
      ctx.records.value = ctx.records.value.filter((r) => !idSet.has(r.id));
      detailRemove(ctx.recordDetails, ids);
      if (ctx.selectedId.value !== null && ctx.selectedIds.value.has(ctx.selectedId.value)) {
        ctx.selectedId.value = null;
      }
      ctx.selectedIds.value = new Set();
      ctx.batchMode.value = false;
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Batch permanent delete failed:", e);
      throw e;
    }
  }

  async function emptyTrash() {
    try {
      await invoke("empty_trash");
      ctx.invalidateLoads(); // invalidate in-flight list loads
      ctx.records.value = [];
      ctx.selectedId.value = null;
      ctx.selectedIds.value = new Set();
      ctx.batchMode.value = false;
      ctx.trashCount.value = 0;
      ctx.recordDetails.value = new Map();
      ctx.scheduleLoadStats();
      await ctx.loadTrashCount();
    } catch (e) {
      console.error("Empty trash failed:", e);
      throw e;
    }
  }

  return {
    pasteRecord,
    batchFavorite,
    deleteRecord,
    toggleFavorite,
    togglePin,
    setAlias,
    deleteBatch,
    restoreRecord,
    restoreRecordsBatch,
    permanentlyDeleteRecord,
    permanentlyDeleteRecordsBatch,
    emptyTrash,
  };
}
