<template>
  <div class="window-app panel-surface">
    <!-- Title Bar -->
    <div class="titlebar" data-tauri-drag-region @dblclick="onTitlebarDblClick">
      <div class="titlebar-left">
        <span class="titlebar-title">剪贴板管理</span>
        <span class="titlebar-version">v0.1.0</span>
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
            <div class="list-sort" title="当前按最近更新排序">最新在前</div>
            <button
              class="list-header-btn"
              :class="{ active: clipboardStore.batchMode }"
              title="批量操作"
              @click="toggleBatchMode"
            ><AppIcon name="batch" :size="14" /></button>
          </div>
        </div>

        <Transition name="fade">
          <div v-if="clipboardStore.batchMode" class="batch-bar">
            <div class="batch-info">
              已选择 <strong>{{ clipboardStore.selectedIds.size }}</strong> 项
            </div>
            <div class="batch-actions">
              <button class="batch-btn" @click="batchCopy"><AppIcon name="copy" :size="13" /> 复制</button>
              <button class="batch-btn" @click="batchFavorite"><AppIcon name="star" :size="13" /> 收藏</button>
              <button class="batch-btn danger" @click="batchDelete"><AppIcon name="trash" :size="13" /> 删除</button>
              <button class="batch-btn" @click="toggleBatchMode"><AppIcon name="close" :size="13" /></button>
            </div>
          </div>
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
import AppIcon from "./icons/AppIcon.vue";
import WindowControls from "./WindowControls.vue";
import { useClipboardStore } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useBatchActions } from "../composables/useBatchActions";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Tag } from "../types";

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode, batchCopy, batchFavorite, batchDelete } = useBatchActions();
const appWindow = getCurrentWindow();

defineEmits<{
  (e: "openSettings"): void;
}>();

useClipboardHotkeys({ allowCloseOnEscape: false });

const tagDialogVisible = ref(false);
const tagDialogMode = ref<"create" | "assign" | "edit">("create");
const editingTag = ref<Tag | null>(null);

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
    await clipboardStore.emptyTrash();
    toast("回收站已清空", "success");
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

.titlebar-version {
  font-size: 0.66rem;
  font-weight: 500;
  color: var(--text-tertiary);
  padding: 2px 7px;
  background: var(--bg-surface);
  border-radius: 20px;
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
  font-size: 0.75rem;
  color: var(--text-tertiary);
  padding: 4px 0;
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

.batch-bar {
  padding: 8px 16px;
  background: var(--accent-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, transparent);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.batch-info {
  font-size: 0.72rem;
  color: var(--accent);
  font-weight: 500;
}

.batch-actions {
  display: flex;
  gap: 6px;
}

.batch-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: 0.69rem;
  font-weight: 500;
  background: var(--bg-surface);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  cursor: pointer;
}

.batch-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.batch-btn.danger {
  background: var(--danger-soft);
  color: var(--danger);
}
</style>
