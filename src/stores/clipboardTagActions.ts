/**
 * Tag-related store actions extracted from clipboard.ts to reduce file size.
 * The store public API is unchanged — methods are spread back into the store return.
 */
import type { Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRecord, Tag } from "../types";
import { featureEnabled } from "../composables/useFeature";
import { detailUpsert } from "./clipboardList";

export interface TagActionsCtx {
  tags: Ref<Tag[]>;
  records: Ref<ClipboardRecord[]>;
  activeTag: Ref<string | null>;
  activeFilter: Ref<string>;
  trashFilter: Ref<boolean>;
  selectedId: Ref<number | null>;
  recordDetails: Ref<Map<number, ClipboardRecord>>;
  patchRecord: (id: number, patch: Partial<ClipboardRecord>) => void;
  patchRecordsBatch: (patches: Map<number, Partial<ClipboardRecord>>) => void;
  reloadList: () => void;
  /** Re-rank records whose `updated_at` the backend bumped (record-level tag
   * link changes). Tag *definitions* (rename/color) no longer bump records —
   * they sync standalone via tags.json — so they don't use this. */
  reorderForUpdates: (ids: number[]) => void;
}

const TAGS_DEBOUNCE_MS = 350;

export function createTagActions(ctx: TagActionsCtx) {
  let tagsLoadTimer: ReturnType<typeof setTimeout> | null = null;

  /** Coalesce rapid get_all_tags calls (filter flips, auto-tag bursts, assign dialog). */
  function scheduleLoadTags() {
    if (!featureEnabled("tags")) return;
    if (tagsLoadTimer) clearTimeout(tagsLoadTimer);
    tagsLoadTimer = setTimeout(() => {
      tagsLoadTimer = null;
      void loadTags();
    }, TAGS_DEBOUNCE_MS);
  }

  async function loadTags() {
    if (!featureEnabled("tags")) {
      ctx.tags.value = [];
      return;
    }
    try {
      const favoritesOnly = !ctx.trashFilter.value && ctx.activeFilter.value === "favorites";
      const contentType =
        !ctx.trashFilter.value && !favoritesOnly && ctx.activeFilter.value !== "all"
          ? ctx.activeFilter.value
          : null;
      ctx.tags.value = await invoke<Tag[]>("get_all_tags", {
        content_type: contentType,
        favorites_only: favoritesOnly,
      });
    } catch (e) {
      console.error("Failed to load tags:", e);
    }
  }

  async function createTag(name: string, color: string) {
    try {
      await invoke<Tag>("create_tag", { name, color });
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to create tag:", e);
      throw e;
    }
  }

  async function deleteTag(id: number) {
    try {
      const existing = ctx.tags.value.find((t) => t.id === id);
      await invoke("delete_tag", { id });
      if (existing) {
        const patches = new Map<number, Partial<ClipboardRecord>>();
        for (const record of ctx.records.value) {
          if (record.tags.includes(existing.name)) {
            patches.set(record.id, { tags: record.tags.filter((t) => t !== existing.name) });
          }
        }
        ctx.patchRecordsBatch(patches);
        if (ctx.activeTag.value === existing.name) {
          ctx.activeTag.value = null;
        }
      }
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to delete tag:", e);
      throw e;
    }
  }

  async function updateTag(id: number, name: string, color: string) {
    try {
      const existing = ctx.tags.value.find((t) => t.id === id);
      const oldName = existing?.name;
      await invoke("update_tag", { id, name, color });
      if (oldName && oldName !== name) {
        const patches = new Map<number, Partial<ClipboardRecord>>();
        for (const record of ctx.records.value) {
          const idx = record.tags.indexOf(oldName);
          if (idx !== -1) {
            const nextTags = [...record.tags];
            nextTags[idx] = name;
            patches.set(record.id, { tags: nextTags });
          }
        }
        ctx.patchRecordsBatch(patches);
        if (ctx.activeTag.value === oldName) {
          ctx.activeTag.value = name;
        }
        // NOTE: renames no longer bump `records.updated_at` (tag definitions
        // sync standalone via tags.json), so no re-rank is needed here.
      }
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to update tag:", e);
      throw e;
    }
  }

  async function addTagToRecord(recordId: number, tagId: number, tagName: string) {
    try {
      await invoke("add_tag_to_record", { record_id: recordId, tag_id: tagId });
      const record = ctx.records.value.find((r) => r.id === recordId);
      if (record && !record.tags.includes(tagName)) {
        ctx.patchRecord(recordId, { tags: [...record.tags, tagName] });
        // Rust bumped updated_at only when the link was newly inserted.
        ctx.reorderForUpdates([recordId]);
      }
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to add tag to record:", e);
      throw e;
    }
  }

  async function removeTagFromRecord(recordId: number, tagId: number, tagName: string) {
    try {
      await invoke("remove_tag_from_record", { record_id: recordId, tag_id: tagId });
      const record = ctx.records.value.find((r) => r.id === recordId);
      if (record) {
        const hadTag = record.tags.includes(tagName);
        ctx.patchRecord(recordId, {
          tags: record.tags.filter((t) => t !== tagName),
        });
        // Rust bumped updated_at only when a link was deleted.
        if (hadTag) ctx.reorderForUpdates([recordId]);
      }
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to remove tag from record:", e);
      throw e;
    }
  }

  /** Replace all tags on a record in one IPC/DB transaction. */
  async function setRecordTags(recordId: number, tagIds: number[], tagNames: string[]) {
    try {
      await invoke("set_record_tags", { record_id: recordId, tag_ids: tagIds });
      const record = ctx.records.value.find((r) => r.id === recordId);
      // Rust bumps updated_at only when the tag set actually changed.
      const changed =
        !record ||
        record.tags.length !== tagNames.length ||
        record.tags.some((t) => !tagNames.includes(t));
      if (record) {
        ctx.patchRecord(recordId, { tags: [...tagNames] });
      }
      if (changed) ctx.reorderForUpdates([recordId]);
      detailUpsert(ctx.recordDetails, recordId, { tags: [...tagNames] });
      scheduleLoadTags();
    } catch (e) {
      console.error("Failed to set record tags:", e);
      throw e;
    }
  }

  function filterByTag(tagName: string | null) {
    // Toggle off when clicking the same tag again; keep type/favorites filter.
    if (tagName && ctx.activeTag.value === tagName) {
      ctx.activeTag.value = null;
    } else {
      ctx.activeTag.value = tagName;
    }
    ctx.selectedId.value = null;
    ctx.reloadList();
  }

  return {
    scheduleLoadTags,
    loadTags,
    createTag,
    deleteTag,
    updateTag,
    addTagToRecord,
    removeTagFromRecord,
    setRecordTags,
    filterByTag,
  };
}
