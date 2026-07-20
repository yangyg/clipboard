<template>
  <aside class="sidebar">
    <!-- Navigation: Categories (includes favorites as a view filter) -->
    <nav class="sidebar-section" aria-label="分类">
      <div class="sidebar-label">分类</div>
      <button
        v-for="item in categoryItems"
        :key="item.key"
        type="button"
        class="nav-item"
        :class="{ active: props.activeCategory === item.key }"
        :aria-current="props.activeCategory === item.key ? 'page' : undefined"
        @click="selectCategory(item.key)"
      >
        <span class="nav-icon">
          <AppIcon
            :name="item.icon"
            :size="15"
            :fill="props.activeCategory === item.key && item.icon === 'star' ? 'currentColor' : 'none'"
          />
        </span>
        <span class="nav-label">{{ item.label }}</span>
        <span v-if="item.count !== undefined" class="nav-count">{{ item.count }}</span>
      </button>
    </nav>

    <!-- Trash: separate from categories / favorites -->
    <nav class="sidebar-section" aria-label="回收站">
      <button
        type="button"
        class="nav-item"
        :class="{ active: props.activeCategory === 'trash' }"
        :aria-current="props.activeCategory === 'trash' ? 'page' : undefined"
        @click="selectCategory('trash')"
      >
        <span class="nav-icon"><AppIcon name="trash" :size="15" /></span>
        <span class="nav-label">回收站</span>
        <span class="nav-count">{{ clipboardStore.trashCount }}</span>
      </button>
    </nav>

    <!-- Tags Section -->
    <div class="sidebar-section sidebar-tags-section">
      <div class="sidebar-label">标签管理</div>
      <div class="tags-list" role="list">
        <button
          v-for="tag in clipboardStore.tags"
          :key="tag.id"
          type="button"
          class="tag-item"
          :class="{ active: props.activeTag === tag.name }"
          :aria-pressed="props.activeTag === tag.name"
          @click="selectTag(tag.name)"
          @contextmenu.prevent.stop="showTagMenu($event, tag)"
        >
          <span class="tag-dot" :style="{ background: tag.color }"></span>
          <span class="tag-name">{{ tag.name }}</span>
          <span class="tag-count">{{ tag.count }}</span>
        </button>
      </div>
      <button type="button" class="tag-add" @click="$emit('addTag')">
        <AppIcon name="plus" :size="13" /> 新建标签
      </button>
    </div>

    <!-- Bottom Actions -->
    <div class="sidebar-bottom">
      <button
        type="button"
        class="sidebar-icon-btn"
        :class="{ 'sidebar-icon-btn-warning': clipboardStore.pauseCapture }"
        :aria-pressed="clipboardStore.pauseCapture"
        :aria-label="clipboardStore.pauseCapture ? '恢复捕获' : '暂停捕获'"
        :title="clipboardStore.pauseCapture ? '恢复捕获' : '暂停捕获'"
        @click="clipboardStore.togglePauseCapture()"
      >
        <AppIcon :name="clipboardStore.pauseCapture ? 'play' : 'pause'" :size="15" />
      </button>
      <button
        type="button"
        class="sidebar-icon-btn"
        :aria-label="themeToggleLabel"
        :title="themeToggleLabel"
        @click="toggleTheme"
      >
        <AppIcon :name="themeToggleIcon" :size="15" />
      </button>
      <button
        type="button"
        class="sidebar-icon-btn"
        aria-label="设置"
        title="设置"
        @click="$emit('openSettings')"
      >
        <AppIcon name="settings" :size="15" />
      </button>
    </div>

    <!-- Tag context menu -->
    <div
      v-if="tagMenu.visible"
      class="context-menu"
      :style="{ left: tagMenu.x + 'px', top: tagMenu.y + 'px' }"
      @click.stop
    >
      <div class="ctx-item" @click="onEdit">
        <span class="ctx-icon"><AppIcon name="edit" :size="14" /></span>编辑
      </div>
      <div class="ctx-sep"></div>
      <div class="ctx-item danger" @click="onDelete">
        <span class="ctx-icon"><AppIcon name="trash" :size="14" /></span>删除
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import type { Tag } from "../types";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();

/** Quick toggle: light ↔ dark; oled/system first click → light. */
const offeringLight = computed(() => settingsStore.settings.theme !== "light");
const themeToggleIcon = computed<AppIconName>(() => (offeringLight.value ? "sun" : "moon"));
const themeToggleLabel = computed(() => (offeringLight.value ? "浅色模式" : "深色模式"));

function toggleTheme() {
  const next = settingsStore.settings.theme === "light" ? "dark" : "light";
  settingsStore.updateSetting("theme", next);
}

const props = defineProps<{
  activeCategory?: string;
  activeTag?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:activeCategory", value: string): void;
  (e: "update:activeTag", value: string | null): void;
  (e: "openSettings"): void;
  (e: "addTag"): void;
  (e: "editTag", tag: Tag): void;
  (e: "deleteTag", tag: Tag): void;
}>();

const tagMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  tag: null as Tag | null,
});

const categoryItems = computed(() => [
  { key: "all", icon: "clipboard" as AppIconName, label: "全部剪贴板", count: clipboardStore.filterCounts.all },
  { key: "text", icon: "type" as AppIconName, label: "文本", count: clipboardStore.filterCounts.text },
  { key: "image", icon: "image" as AppIconName, label: "图片", count: clipboardStore.filterCounts.image },
  { key: "file", icon: "file" as AppIconName, label: "文件", count: clipboardStore.filterCounts.file },
  { key: "link", icon: "link" as AppIconName, label: "链接", count: clipboardStore.filterCounts.link },
  { key: "code", icon: "code" as AppIconName, label: "代码", count: clipboardStore.filterCounts.code },
  { key: "favorites", icon: "star" as AppIconName, label: "收藏夹", count: clipboardStore.filterCounts.favorites },
]);

function selectCategory(key: string) {
  emit("update:activeCategory", key);
  // Trash is exclusive; other categories keep the current tag (AND).
  if (key === "trash") {
    emit("update:activeTag", null);
  }
}

function selectTag(name: string) {
  // Keep category selection — type/favorites AND tag combine.
  emit("update:activeTag", name);
}

function showTagMenu(e: MouseEvent, tag: Tag) {
  const menuW = 140;
  const menuH = 80;
  tagMenu.x = Math.min(e.clientX, window.innerWidth - menuW - 8);
  tagMenu.y = Math.min(e.clientY, window.innerHeight - menuH - 8);
  tagMenu.tag = tag;
  tagMenu.visible = true;
}

function closeTagMenu() {
  tagMenu.visible = false;
  tagMenu.tag = null;
}

function onEdit() {
  if (tagMenu.tag) emit("editTag", tagMenu.tag);
  closeTagMenu();
}

function onDelete() {
  if (tagMenu.tag) emit("deleteTag", tagMenu.tag);
  closeTagMenu();
}

function onGlobalClick() {
  if (tagMenu.visible) closeTagMenu();
}

onMounted(() => {
  window.addEventListener("click", onGlobalClick);
});

onUnmounted(() => {
  window.removeEventListener("click", onGlobalClick);
});
</script>

<style scoped>
.sidebar {
  width: 200px;
  min-width: 200px;
  background: var(--bg-elevated);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0;
  position: relative;
}

.sidebar-section {
  padding: 12px 10px 4px;
  flex-shrink: 0;
}

.sidebar-tags-section {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-label {
  font-size: 0.625rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-tertiary);
  padding: 0 8px 6px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 8px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-secondary);
  font-size: 0.78rem;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.nav-icon {
  display: flex;
  width: 18px;
  align-items: center;
  justify-content: center;
  color: inherit;
}

.nav-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav-count {
  font-size: 0.625rem;
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.nav-item.active .nav-count {
  color: var(--accent);
}

.tags-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.tag-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 8px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 0.75rem;
  color: var(--text-secondary);
  transition: background var(--transition-fast);
}

.tag-item:hover {
  background: var(--bg-hover);
}

.tag-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.tag-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tag-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag-count {
  font-size: 0.625rem;
  color: var(--text-tertiary);
}

.tag-add {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 8px;
  margin-top: 4px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  color: var(--text-tertiary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.tag-add:hover {
  background: var(--bg-hover);
  color: var(--accent);
}

.sidebar-bottom {
  padding: 8px 10px 12px;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-around;
  gap: 4px;
}

.sidebar-icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  height: 32px;
  padding: 0;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.sidebar-icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sidebar-icon-btn-warning {
  color: var(--warning);
}

.sidebar-icon-btn-warning:hover {
  color: var(--warning);
  background: var(--warning-soft);
}

.context-menu {
  position: fixed;
  width: 140px;
  background: var(--bg-surface);
  border: 1px solid var(--border-default, var(--border-subtle));
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  padding: 6px;
  z-index: 200;
}

.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  font-size: 12px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.ctx-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.ctx-item.danger {
  color: var(--danger);
}

.ctx-item.danger:hover {
  background: var(--danger-soft);
}

.ctx-icon {
  width: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ctx-sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--border-subtle);
}
</style>
