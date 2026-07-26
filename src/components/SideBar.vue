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
        :class="{
          active: props.activeCategory === item.key,
          'has-cat-color': !!item.color,
        }"
        :style="item.color ? { '--cat-color': item.color } : undefined"
        :aria-current="props.activeCategory === item.key ? 'page' : undefined"
        :title="item.label"
        :aria-label="item.label"
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
        title="回收站"
        aria-label="回收站"
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
          v-for="tag in primaryTags"
          :key="tag.id"
          type="button"
          class="tag-item"
          :class="{ active: props.activeTag === tag.name }"
          :aria-pressed="props.activeTag === tag.name"
          :title="tagTitle(tag)"
          :aria-label="tagAriaLabel(tag)"
          @click="selectTag(tag.name)"
          @contextmenu.prevent.stop="showTagMenu($event, tag)"
        >
          <span class="tag-dot" :style="{ background: tag.color }"></span>
          <span class="tag-name">{{ tag.name }}</span>
          <span
            v-if="tag.is_auto"
            class="tag-auto-icon"
            title="自动打标规则创建"
            aria-hidden="true"
          ><AppIcon name="sparkles" :size="11" /></span>
          <span class="tag-count">{{ tag.count }}</span>
        </button>

        <template v-if="moreTags.length > 0">
          <button
            type="button"
            class="tag-more-toggle"
            :aria-expanded="moreTagsOpen"
            :aria-label="moreTagsOpen ? `收起空标签，共 ${moreTags.length} 个` : `更多空标签，共 ${moreTags.length} 个`"
            @click="moreTagsOpen = !moreTagsOpen"
          >
            <span class="tag-more-label">{{ moreTagsOpen ? "收起" : "更多" }}</span>
            <span class="tag-more-meta">{{ moreTags.length }}</span>
          </button>
          <template v-if="moreTagsOpen">
            <button
              v-for="tag in moreTags"
              :key="tag.id"
              type="button"
              class="tag-item tag-item-muted"
              :class="{ active: props.activeTag === tag.name }"
              :aria-pressed="props.activeTag === tag.name"
              :title="tagTitle(tag)"
              :aria-label="tagAriaLabel(tag)"
              @click="selectTag(tag.name)"
              @contextmenu.prevent.stop="showTagMenu($event, tag)"
            >
              <span class="tag-dot" :style="{ background: tag.color }"></span>
              <span class="tag-name">{{ tag.name }}</span>
              <span
                v-if="tag.is_auto"
                class="tag-auto-icon"
                title="自动打标规则创建"
                aria-hidden="true"
              ><AppIcon name="sparkles" :size="11" /></span>
              <span class="tag-count">{{ tag.count }}</span>
            </button>
          </template>
        </template>
      </div>
        <button type="button" class="tag-add" title="新建标签" aria-label="新建标签" @click="$emit('addTag')">
        <AppIcon name="plus" :size="13" /> <span class="tag-add-label">新建标签</span>
      </button>
    </div>

    <!-- Bottom Actions: quick-menu (left) + settings (right) -->
    <div class="sidebar-bottom">
      <button
        ref="quickMenuAnchorEl"
        type="button"
        class="sidebar-icon-btn sidebar-icon-btn-grow"
        :class="{ 'sidebar-icon-btn-warning': clipboardStore.pauseCapture }"
        aria-haspopup="menu"
        :aria-expanded="quickMenu.visible"
        aria-label="快捷菜单"
        title="主题与监控"
        @click="toggleQuickMenu"
      >
        <AppIcon name="menu" :size="15" />
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

    <ContextMenu
      :visible="quickMenu.visible"
      :x="quickMenu.x"
      :y="quickMenu.y"
      :width="180"
      :items="quickMenuItems"
      placement="top"
      :anchor="quickMenuAnchorEl"
      @close="closeQuickMenu"
      @select="onQuickMenuSelect"
    />

    <ContextMenu
      :visible="tagMenu.visible"
      :x="tagMenu.x"
      :y="tagMenu.y"
      :width="140"
      :items="tagMenuItems"
      @close="closeTagMenu"
      @select="onTagMenuSelect"
    />
  </aside>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "../composables/useToast";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu.vue";
import type { Tag } from "../types";
import type { WebDavSyncResult } from "../types";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { toast } = useToast();

const props = defineProps<{
  activeCategory?: string;
  activeTag?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:activeCategory", value: string): void;
  (e: "update:activeTag", value: string | null): void;
  (e: "openSettings", section?: string): void;
  (e: "addTag"): void;
  (e: "editTag", tag: Tag): void;
  (e: "deleteTag", tag: Tag): void;
}>();

const moreTagsOpen = ref(false);

/** Count > 0, or currently selected (so a zero-count filter stays visible). */
const primaryTags = computed(() =>
  clipboardStore.tags.filter(
    (t) => t.count > 0 || (props.activeTag != null && t.name === props.activeTag),
  ),
);

const moreTags = computed(() =>
  clipboardStore.tags.filter(
    (t) => t.count === 0 && !(props.activeTag != null && t.name === props.activeTag),
  ),
);

function tagTitle(tag: Tag): string {
  return tag.is_auto ? `${tag.name}（自动打标规则创建）` : tag.name;
}

function tagAriaLabel(tag: Tag): string {
  return tag.is_auto ? `${tag.name}，自动打标规则创建` : tag.name;
}

const tagMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  tag: null as Tag | null,
});

const tagMenuItems: ContextMenuItem[] = [
  { id: "edit", label: "编辑", icon: "edit" },
  { id: "delete", label: "删除", icon: "trash", danger: true, separatorBefore: true },
];

const categoryItems = computed(() => [
  { key: "all", icon: "clipboard" as AppIconName, label: "全部剪贴板", count: clipboardStore.filterCounts.all },
  { key: "text", icon: "type" as AppIconName, label: "文本", count: clipboardStore.filterCounts.text, color: "var(--type-text)" },
  { key: "image", icon: "image" as AppIconName, label: "图片", count: clipboardStore.filterCounts.image, color: "var(--type-image)" },
  { key: "file", icon: "file" as AppIconName, label: "文件", count: clipboardStore.filterCounts.file, color: "var(--type-file)" },
  { key: "link", icon: "link" as AppIconName, label: "链接", count: clipboardStore.filterCounts.link, color: "var(--type-link)" },
  { key: "code", icon: "code" as AppIconName, label: "代码", count: clipboardStore.filterCounts.code, color: "var(--type-code)" },
  { key: "favorites", icon: "star" as AppIconName, label: "收藏夹", count: clipboardStore.filterCounts.favorites, color: "var(--warning)" },
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
  tagMenu.x = e.clientX;
  tagMenu.y = e.clientY;
  tagMenu.tag = tag;
  tagMenu.visible = true;
}

function closeTagMenu() {
  tagMenu.visible = false;
  tagMenu.tag = null;
}

function onTagMenuSelect(id: string) {
  if (!tagMenu.tag) return;
  if (id === "edit") emit("editTag", tagMenu.tag);
  if (id === "delete") emit("deleteTag", tagMenu.tag);
}

/* ── Quick menu (theme + capture toggle) ── */

const quickMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
});

const webdavSyncing = ref(false);

const quickMenuAnchorEl = ref<HTMLElement | null>(null);

const quickMenuItems = computed<ContextMenuItem[]>(() => [
  {
    id: "theme-toggle",
    label: "外观",
    icon: "palette",
    toggle: {
      value: settingsStore.settings.theme !== "light",
      labels: ["浅色", "深色"],
    },
  },
  {
    id: "capture-toggle",
    label: clipboardStore.pauseCapture ? "恢复捕获" : "暂停捕获",
    icon: (clipboardStore.pauseCapture ? "play" : "pause") as AppIconName,
    separatorBefore: true,
  },
  {
    id: "webdav-sync",
    label: webdavSyncing.value ? "同步中…" : "WebDAV 同步",
    icon: "cloud",
    separatorBefore: true,
  },
  {
    id: "help",
    label: "帮助",
    icon: "help",
  },
]);

function toggleQuickMenu(e: MouseEvent) {
  if (quickMenu.visible) {
    quickMenu.visible = false;
    return;
  }
  const target = e.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  quickMenu.x = rect.left;
  quickMenu.y = rect.top; // ContextMenu clamps into viewport
  quickMenu.visible = true;
}

function closeQuickMenu() {
  quickMenu.visible = false;
}

function onQuickMenuSelect(id: string) {
  if (id === "theme-toggle") {
    const next = settingsStore.settings.theme === "light" ? "dark" : "light";
    settingsStore.updateSetting("theme", next);
    return;
  }
  if (id === "capture-toggle") {
    clipboardStore.togglePauseCapture();
    return;
  }
  if (id === "webdav-sync") {
    webdavSync();
    return;
  }
  if (id === "help") {
    quickMenu.visible = false;
    emit("openSettings", "help");
    return;
  }
}

function isWebDavConfigured(): boolean {
  const s = settingsStore.settings;
  const urlOk = /^https?:\/\/.+/i.test(s.webdav_url.trim());
  return urlOk && s.webdav_username.trim().length > 0 && s.webdav_password.length > 0;
}

async function webdavSync() {
  if (webdavSyncing.value) return;
  if (!isWebDavConfigured()) {
    toast("请先在设置中配置 WebDAV 同步", "warning");
    quickMenu.visible = false;
    emit("openSettings", "data");
    return;
  }
  webdavSyncing.value = true;
  try {
    await settingsStore.saveSettings();
    const result = await invoke<WebDavSyncResult>("webdav_sync");
    await settingsStore.loadSettings();
    await clipboardStore.loadRecords();
    await clipboardStore.loadStats();
    toast(result.message || "WebDAV 同步完成", "success");
  } catch (e) {
    toast(`WebDAV 同步失败：${String(e)}`, "error");
    quickMenu.visible = false;
    emit("openSettings", "data");
  } finally {
    webdavSyncing.value = false;
    quickMenu.visible = false;
  }
}
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
  letter-spacing: 0.02em;
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

/* Type / favorites: active uses category color instead of accent */
.nav-item.has-cat-color.active {
  color: var(--cat-color);
  background: color-mix(in srgb, var(--cat-color) 16%, transparent);
}

.nav-icon {
  display: flex;
  width: 18px;
  align-items: center;
  justify-content: center;
  color: inherit;
  flex-shrink: 0;
}

.nav-item.has-cat-color .nav-icon {
  color: var(--cat-color);
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
  color: inherit;
  opacity: 0.85;
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

.tag-auto-icon {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--accent);
  opacity: 0.9;
}

.tag-count {
  font-size: 0.625rem;
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.tag-more-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 6px 8px;
  margin-top: 2px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-tertiary);
  font-size: 0.6875rem;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.tag-more-toggle:hover {
  background: var(--bg-hover);
  color: var(--text-secondary);
}

.tag-more-label {
  flex: 1;
  min-width: 0;
}

.tag-more-meta {
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
}

.tag-item-muted {
  opacity: 0.85;
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
  flex-shrink: 0;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
}

.sidebar-icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
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

/* Left (quick-menu) button fills remaining width for a larger click target */
.sidebar-icon-btn-grow {
  flex: 1;
  justify-content: flex-start;
  padding-left: 10px;
}

.sidebar-icon-btn-warning {
  color: var(--warning);
}

.sidebar-icon-btn-warning:hover {
  color: var(--warning);
  background: var(--warning-soft);
}

/* Narrow window: icon rail */
@media (max-width: 720px) {
  .sidebar {
    width: 56px;
    min-width: 56px;
  }

  .sidebar-label,
  .nav-label,
  .nav-count,
  .tag-name,
  .tag-count,
  .tag-auto-icon,
  .tag-more-label {
    display: none;
  }

  .nav-item,
  .tag-item {
    justify-content: center;
    padding: 8px;
  }

  .tag-add {
    justify-content: center;
    padding: 8px;
  }

  .tag-add-label {
    display: none;
  }

  .sidebar-bottom {
    flex-direction: column;
    align-items: center;
  }

  .sidebar-icon-btn-grow {
    flex: 0 0 auto;
    width: 36px;
    justify-content: center;
    padding-left: 0;
  }
}
</style>
