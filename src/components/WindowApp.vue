<template>
  <div class="window-app">
    <!-- Title Bar -->
    <div class="titlebar" data-tauri-drag-region>
      <div class="titlebar-left">
        <div class="titlebar-logo">
          <div class="logo-icon">
            <span class="logo-glyph">📋</span>
          </div>
          <span>ClipBoard</span>
        </div>
        <span class="titlebar-version">v1.0.0</span>
      </div>

      <div class="titlebar-center" data-tauri-drag-region>
        <SearchBar compact />
      </div>

      <div class="titlebar-actions">
        <CaptureStatus />
      </div>
    </div>

    

    <!-- Three-Column Layout -->
    <div class="window-body">
      <!-- Left Sidebar -->
      <SideBar
        :activeCategory="activeCategory"
        :activeTag="clipboardStore.activeTag"
        @update:activeCategory="onCategoryChange"
        @update:activeTag="onTagChange"
        @openSettings="$emit('openSettings')"
        @addTag="onAddTag"
      />

      <!-- Center: Record List -->
      <div class="center-column">
        <!-- List Header -->
        <div class="list-header">
          <span class="list-title">{{ categoryTitle }}</span>
          <div class="list-header-right">
            <button
              v-if="clipboardStore.trashFilter && clipboardStore.trashCount > 0"
              class="empty-trash-btn"
              @click="onEmptyTrash"
            >清空回收站</button>
            <div class="list-sort">
              最新在前 <span class="sort-arrow">▼</span>
            </div>
          </div>
        </div>

        <!-- Record List -->
        <RecordList />
      </div>
    </div>

    <!-- Tag Dialog -->
    <TagDialog
      :visible="tagDialogVisible"
      :mode="tagDialogMode"
      @close="tagDialogVisible = false"
      @switchToCreate="tagDialogMode = 'create'"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import SideBar from "./SideBar.vue";
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import TagDialog from "./TagDialog.vue";
import CaptureStatus from "./CaptureStatus.vue";
import { useClipboardStore } from "../stores/clipboard";

const clipboardStore = useClipboardStore();

defineEmits<{
  (e: "openSettings"): void;
}>();

const activeCategory = ref("all");
const tagDialogVisible = ref(false);
const tagDialogMode = ref<"create" | "assign">("create");

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

const categoryTitle = computed(() => {
  if (clipboardStore.trashFilter) return "回收站";
  if (clipboardStore.activeTag) return `标签: ${clipboardStore.activeTag}`;
  return CATEGORY_TITLES[activeCategory.value] ?? "全部剪贴板";
});

function onCategoryChange(key: string) {
  activeCategory.value = key;
  if (key === "trash") {
    clipboardStore.setTrashFilter(true);
    // search("") clears searchQuery and calls loadRecords internally
    clipboardStore.search("");
    return;
  }
  const comingFromTrash = clipboardStore.trashFilter;
  clipboardStore.setTrashFilter(false);
  const mapping: Record<string, "all" | "text" | "code" | "link" | "image" | "file" | "favorites"> = {
    all: "all",
    text: "text",
    image: "image",
    file: "file",
    link: "link",
    code: "code",
    favorites: "favorites",
  };
  clipboardStore.setFilter(mapping[key] ?? "all");
  if (comingFromTrash) {
    clipboardStore.search("");
  }
}

function onTagChange(tagName: string | null) {
  clipboardStore.filterByTag(tagName);
}

function onAddTag() {
  tagDialogMode.value = "create";
  tagDialogVisible.value = true;
}

async function onEmptyTrash() {
  if (confirm("确定要清空回收站吗？所有已删除的记录将被永久删除，此操作不可恢复。")) {
    await clipboardStore.emptyTrash();
  }
}

onMounted(() => {
  clipboardStore.loadTags();
});
</script>

<style scoped>
.window-app {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-surface);
  color: var(--text-primary);
}

/* Title Bar */
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

.titlebar-logo {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 700;
  font-size: 13.5px;
  letter-spacing: -0.02em;
  color: var(--text-primary);
}

.logo-icon {
  width: 22px;
  height: 22px;
  background: linear-gradient(135deg, var(--accent), var(--accent-light, #6b85fa));
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.logo-glyph {
  font-size: 11px;
  color: #fff;
}

.titlebar-version {
  font-size: 10.5px;
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
}

.titlebar-btn {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.titlebar-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.titlebar-btn.close:hover {
  background: var(--danger);
  color: #fff;
}

/* Three-Column Body */
.window-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

/* Center Column */
.center-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* List Header */
.list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px 6px;
  border-bottom: 1px solid var(--border-light, var(--border-subtle));
  flex-shrink: 0;
}

.list-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
}

.list-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.empty-trash-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-weight: 500;
  background: var(--danger-soft);
  color: var(--danger);
  border: 1px solid rgba(248, 113, 113, 0.2);
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.empty-trash-btn:hover {
  background: rgba(248, 113, 113, 0.2);
}

.list-sort {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.list-sort:hover {
  background: var(--bg-hover);
}

.sort-arrow {
  font-size: 11px;
}
</style>
