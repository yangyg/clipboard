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
        @openSettings="(section?: string) => $emit('openSettings', section)"
        @addTag="onAddTag"
        @editTag="onEditTag"
        @deleteTag="onDeleteTag"
      />

      <div class="center-column">
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
import WindowControls from "./WindowControls.vue";
import { useClipboardStore } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Tag } from "../types";

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();
const appWindow = getCurrentWindow();

defineEmits<{
  (e: "openSettings", section?: string): void;
}>();

useClipboardHotkeys({ allowCloseOnEscape: false });

const tagDialogVisible = ref(false);
const tagDialogMode = ref<"create" | "assign" | "edit">("create");
const editingTag = ref<Tag | null>(null);

async function onTitlebarDblClick() {
  await appWindow.toggleMaximize();
}

/** Single source of truth: sidebar highlight follows store, not a separate ref. */
const activeCategory = computed(() =>
  clipboardStore.trashFilter ? "trash" : clipboardStore.activeFilter
);

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
    title: t('sidebar.deleteTagTitle'),
    message: t('sidebar.deleteTagMsg', { name: tag.name }),
    confirmText: t('common.delete'),
    danger: true,
  });
  if (!ok) return;
  try {
    await clipboardStore.deleteTag(tag.id);
  } catch {
    toast(t('sidebar.deleteTagFailed'), "error");
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
  padding: 0 var(--space-3);
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border-subtle);
  user-select: none;
  flex-shrink: 0;
  gap: var(--space-3);
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  -webkit-app-region: drag;
}

.titlebar-title {
  font-weight: 700;
  font-size: var(--text-lg);
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
  gap: var(--space-1);
  -webkit-app-region: no-drag;
  flex-shrink: 0;
  align-items: stretch;
  height: 100%;
  margin-right: calc(-1 * var(--space-3));
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
</style>
