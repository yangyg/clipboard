<template>
  <div class="window-app panel-surface">
    <!-- Title Bar -->
    <div class="titlebar" data-tauri-drag-region @dblclick="onTitlebarDblClick">
      <div class="titlebar-left">
        <span class="titlebar-title">ClipVault</span>
      </div>

      <div class="titlebar-center" data-tauri-drag-region>
        <SearchBar compact />
      </div>

      <div class="titlebar-actions">
        <WindowControls />
      </div>
    </div>

    <!-- Three-Column Layout -->
    <div class="window-body">
      <SideBar
        :activeCategory="activeCategory"
        :activeTag="clipboardStore.activeTag"
        @update:activeCategory="onCategoryChange"
        @update:activeTag="onTagChange"
        @openSettings="$emit('openSettings')"
        @addTag="onAddTag"
        @editTag="onEditTag"
        @deleteTag="onDeleteTag"
      />

      <div class="center-column">
        <div class="list-header">
          <span class="list-title">{{ categoryTitle }}</span>
          <div class="list-header-right">
            <button
              v-if="clipboardStore.trashFilter && clipboardStore.trashCount > 0"
              class="empty-trash-btn"
              @click="onEmptyTrash"
            >清空回收站</button>
          </div>
        </div>

        <div class="list-toolbar">
          <div class="list-count">{{ listCountLabel }}</div>
          <div class="list-toolbar-right">
            <select
              class="list-sort"
              :value="clipboardStore.listSort"
              title="列表排序"
              aria-label="列表排序"
              @change="onSortChange"
            >
              <option
                v-for="opt in LIST_SORT_OPTIONS"
                :key="opt.value"
                :value="opt.value"
              >{{ opt.label }}</option>
            </select>
            <button
              type="button"
              class="list-header-btn"
              :class="{ active: clipboardStore.batchMode }"
              title="批量操作"
              aria-label="批量操作"
              :aria-pressed="clipboardStore.batchMode"
              @click="toggleBatchMode"
            ><AppIcon name="batch" :size="14" /></button>
          </div>
        </div>

        <Transition name="fade">
          <BatchBar v-if="clipboardStore.batchMode" />
        </Transition>

        <RecordList />
      </div>
    </div>

    <TagDialog
      :visible="tagDialogVisible"
      :mode="tagDialogMode"
      :editTag="editingTag"
      @close="onTagDialogClose"
      @switchToCreate="tagDialogMode = 'create'"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import SideBar from "./SideBar.vue";
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import TagDialog from "./TagDialog.vue";
import BatchBar from "./BatchBar.vue";
import AppIcon from "./icons/AppIcon.vue";
import WindowControls from "./WindowControls.vue";
import { useClipboardStore, LIST_SORT_OPTIONS, type ListSort } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useBatchActions } from "../composables/useBatchActions";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Tag } from "../types";

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode } = useBatchActions();
const appWindow = getCurrentWindow();

defineEmits<{
  (e: "openSettings"): void;
}>();

useClipboardHotkeys({ allowCloseOnEscape: false });

const tagDialogVisible = ref(false);
const tagDialogMode = ref<"create" | "assign" | "edit">("create");
const editingTag = ref<Tag | null>(null);

function onSortChange(e: Event) {
  const value = (e.target as HTMLSelectElement).value as ListSort;
  clipboardStore.setListSort(value);
}

async function onTitlebarDblClick() {
  await appWindow.toggleMaximize();
}

const CATEGORY_TITLES: Record<string, string> = {
  all: "全部剪贴板",
  text: "文本",
  image: "图片",
  file: "文件",
  link: "链接",
  code: "代码",
  favorites: "收藏夹",
  trash: "回收站",
};

/** Single source of truth: sidebar highlight follows store, not a separate ref. */
const activeCategory = computed(() =>
  clipboardStore.trashFilter ? "trash" : clipboardStore.activeFilter
);

const categoryTitle = computed(() => {
  if (clipboardStore.trashFilter) return "回收站";
  const typeKey = clipboardStore.activeFilter;
  const typePart =
    typeKey !== "all" ? CATEGORY_TITLES[typeKey] ?? typeKey : null;
  const tagPart = clipboardStore.activeTag;
  if (typePart && tagPart) return `${typePart} · ${tagPart}`;
  if (tagPart) return tagPart;
  if (typePart) return typePart;
  return "全部剪贴板";
});

const listCountLabel = computed(() => {
  if (clipboardStore.searchQuery) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? `已找到 ${n}+ 项` : `共 ${n} 项`;
  }
  if (clipboardStore.trashFilter) {
    return `共 ${clipboardStore.trashCount} 项`;
  }
  // Combined or tag-only: stats are not intersection-aware — use loaded page size.
  if (clipboardStore.activeTag) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? `已加载 ${n}+ 项` : `共 ${n} 项`;
  }
  if (clipboardStore.activeFilter === "favorites") {
    return `共 ${clipboardStore.filterCounts.favorites} 项`;
  }
  if (clipboardStore.activeFilter !== "all") {
    return `共 ${clipboardStore.filterCounts[clipboardStore.activeFilter]} 项`;
  }
  return `共 ${clipboardStore.filterCounts.all} 项`;
});

function onCategoryChange(key: string) {
  if (key === "trash") {
    clipboardStore.setTrashFilter(true);
    void clipboardStore.loadRecords();
    return;
  }
  if (clipboardStore.trashFilter) {
    clipboardStore.setTrashFilter(false);
  }
  const mapping: Record<string, "all" | "text" | "code" | "link" | "image" | "file" | "favorites"> = {
    all: "all",
    text: "text",
    image: "image",
    file: "file",
    link: "link",
    code: "code",
    favorites: "favorites",
  };
  // setFilter keeps activeTag (AND combine)
  clipboardStore.setFilter(mapping[key] ?? "all");
}

function onTagChange(tagName: string | null) {
  if (tagName) {
    // Leave trash; keep current type/favorites selection for AND combine.
    if (clipboardStore.trashFilter) {
      clipboardStore.setTrashFilter(false);
    }
    clipboardStore.filterByTag(tagName);
  } else {
    clipboardStore.filterByTag(null);
  }
}

function onAddTag() {
  editingTag.value = null;
  tagDialogMode.value = "create";
  tagDialogVisible.value = true;
}

function onEditTag(tag: Tag) {
  editingTag.value = tag;
  tagDialogMode.value = "edit";
  tagDialogVisible.value = true;
}

function onTagDialogClose() {
  tagDialogVisible.value = false;
  editingTag.value = null;
}

async function onDeleteTag(tag: Tag) {
  const ok = await confirm({
    title: "删除标签",
    message: `确定删除标签「${tag.name}」吗？已关联的记录将移除该标签，此操作不可恢复。`,
    confirmText: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    await clipboardStore.deleteTag(tag.id);
  } catch {
    toast("删除失败", "error");
  }
}

async function onEmptyTrash() {
  const ok = await confirm({
    title: "清空回收站",
    message: "确定要清空回收站吗？所有已删除的记录将被永久删除，此操作不可恢复。",
    confirmText: "清空",
    danger: true,
  });
  if (ok) {
    try {
      await clipboardStore.emptyTrash();
      toast("回收站已清空", "success");
    } catch {
      toast("清空失败", "error");
    }
  }
}

</script>

<style scoped>
.window-app {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--text-primary);
}

.titlebar {
  height: 38px;
  min-height: 38px;
  display: flex;
  align-items: center;
  padding: 0 10px;
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border-subtle);
  user-select: none;
  flex-shrink: 0;
  gap: 12px;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  -webkit-app-region: drag;
}

.titlebar-title {
  font-weight: 700;
  font-size: 0.85rem;
  letter-spacing: -0.02em;
  color: var(--text-primary);
}

.titlebar-center {
  flex: 1;
  display: flex;
  justify-content: center;
  -webkit-app-region: no-drag;
}

.titlebar-actions {
  display: flex;
  gap: 4px;
  -webkit-app-region: no-drag;
  flex-shrink: 0;
  align-items: stretch;
  height: 100%;
  margin-right: -10px;
}

.window-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.center-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px 2px;
  flex-shrink: 0;
}

.list-title {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--text-primary);
}

.list-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 2px 12px 8px 16px;
  border-bottom: 1px solid var(--border-light);
  flex-shrink: 0;
}

.list-count {
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.list-toolbar-right {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}

.empty-trash-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: 0.69rem;
  font-weight: 500;
  background: var(--danger-soft);
  color: var(--danger);
  border: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.empty-trash-btn:hover {
  background: color-mix(in srgb, var(--danger) 20%, transparent);
}

.list-sort {
  height: 26px;
  max-width: 7.5rem;
  padding: 0 6px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: 0.75rem;
  font-family: inherit;
  cursor: pointer;
  outline: none;
  transition: border-color var(--transition-fast), color var(--transition-fast);
}

.list-sort:hover,
.list-sort:focus {
  border-color: var(--accent);
  color: var(--text-primary);
}

.list-header-btn {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.list-header-btn:hover,
.list-header-btn.active {
  background: var(--accent-soft);
  color: var(--accent);
}
</style>
