<template>
  <div class="window-app panel-surface">
    <!-- Title Bar -->
    <div class="titlebar" data-tauri-drag-region @dblclick="onTitlebarDblClick">
      <div class="titlebar-left">
        <span class="titlebar-title">{{ $t('common.appName') }}</span>
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
        :style="!isNarrow ? { width: sidebarWidth + 'px', minWidth: sidebarWidth + 'px' } : undefined"
        :activeCategory="activeCategory"
        :activeTag="clipboardStore.activeTag"
        @update:activeCategory="onCategoryChange"
        @update:activeTag="onTagChange"
        @openSettings="(section?: string) => $emit('openSettings', section)"
        @addTag="onAddTag"
        @editTag="onEditTag"
        @deleteTag="onDeleteTag"
      />
      <div
        v-if="!isNarrow"
        class="resizer"
        :class="{ active: sidebarDragging }"
        role="separator"
        aria-orientation="vertical"
        :aria-valuenow="sidebarWidth"
        :aria-valuemin="120"
        :aria-valuemax="360"
        tabindex="0"
        :aria-label="$t('record.resizeSidebar')"
        @pointerdown="startSidebarResize"
        @keydown="onSidebarResizeKey"
      />

      <div class="center-column">
        <RecordList />
      </div>
    </div>

    <TagDialog
      v-if="tagsEnabled"
      :visible="tagDialogVisible"
      :mode="tagDialogMode"
      :editTag="editingTag"
      @close="onTagDialogClose"
      @switchToCreate="tagDialogMode = 'create'"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import SideBar from "./SideBar.vue";
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import TagDialog from "./TagDialog.vue";
import WindowControls from "./WindowControls.vue";
import { useClipboardStore } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useFeature } from "../features/capabilities";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useColumnResize } from "../composables/useColumnResize";
import type { Tag } from "../types";

const clipboardStore = useClipboardStore();
const tagsEnabled = useFeature("tags");
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();
const appWindow = getCurrentWindow();

defineEmits<{
  (e: "openSettings", section?: string): void;
}>();

useClipboardHotkeys({ allowCloseOnEscape: false });

// --- Sidebar column resize ---
const narrowMq = window.matchMedia("(max-width: 720px)");
const isNarrow = ref(narrowMq.matches);
function onMqChange(e: MediaQueryListEvent) {
  isNarrow.value = e.matches;
}
onMounted(() => narrowMq.addEventListener("change", onMqChange));
onUnmounted(() => narrowMq.removeEventListener("change", onMqChange));

const {
  width: sidebarWidth,
  isDragging: sidebarDragging,
  startResize: startSidebarResize,
  setWidth: setSidebarWidth,
} = useColumnResize({
  storageKey: "clipboard-sidebar-width",
  defaultWidth: 200,
  min: 120,
  max: 360,
  disabled: isNarrow,
});

function onSidebarResizeKey(e: KeyboardEvent) {
  const step = e.shiftKey ? 40 : 16;
  if (e.key === "ArrowLeft") {
    e.preventDefault();
    setSidebarWidth(sidebarWidth.value - step);
  } else if (e.key === "ArrowRight") {
    e.preventDefault();
    setSidebarWidth(sidebarWidth.value + step);
  } else if (e.key === "Home") {
    e.preventDefault();
    setSidebarWidth(120);
  } else if (e.key === "End") {
    e.preventDefault();
    setSidebarWidth(360);
  }
}

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
  if (!tagsEnabled.value) return;
  editingTag.value = null;
  tagDialogMode.value = "create";
  tagDialogVisible.value = true;
}

function onEditTag(tag: Tag) {
  if (!tagsEnabled.value) return;
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
  font-weight: 600;
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

.resizer {
  width: 4px;
  /* Overlay the left column's right edge instead of reserving flex space,
     keeping the layout fully compact. z-index keeps it above the column's
     positioned content so it still receives hover/drag pointer events. */
  margin-left: -4px;
  position: relative;
  z-index: 10;
  cursor: col-resize;
  background: transparent;
  flex-shrink: 0;
  transition: background var(--transition-fast);
  touch-action: none;
}

.resizer:hover,
.resizer.active {
  background: var(--accent);
}

.center-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>
