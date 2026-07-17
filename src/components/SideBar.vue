<template>
  <aside class="sidebar">
    <!-- Navigation: Categories -->
    <nav class="sidebar-section">
      <div class="sidebar-label">分类</div>
      <div
        v-for="item in categoryItems"
        :key="item.key"
        class="nav-item"
        :class="{ active: props.activeCategory === item.key }"
        @click="selectCategory(item.key)"
      >
        <span class="nav-icon"><AppIcon :name="item.icon" :size="15" /></span>
        <span class="nav-label">{{ item.label }}</span>
        <span v-if="item.count !== undefined" class="nav-count">{{ item.count }}</span>
      </div>
    </nav>

    <!-- Special -->
    <nav class="sidebar-section">
      <div class="sidebar-label">收藏</div>
      <div
        v-for="item in specialItems"
        :key="item.key"
        class="nav-item"
        :class="{ active: props.activeCategory === item.key }"
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
      </div>
    </nav>

    <!-- Tags Section -->
    <div class="sidebar-section sidebar-tags-section">
      <div class="sidebar-label">标签管理</div>
      <div class="tags-list">
        <div
          v-for="tag in clipboardStore.tags"
          :key="tag.id"
          class="tag-item"
          :class="{ active: props.activeTag === tag.name }"
          @click="selectTag(tag.name)"
        >
          <span class="tag-dot" :style="{ background: tag.color }"></span>
          <span class="tag-name">{{ tag.name }}</span>
          <span class="tag-count">{{ tag.count }}</span>
        </div>
      </div>
      <div class="tag-add" @click="$emit('addTag')">
        <AppIcon name="plus" :size="13" /> 新建标签
      </div>
    </div>

    <!-- Bottom Actions -->
    <div class="sidebar-bottom">
      <div class="nav-item" @click="$emit('openSettings')">
        <span class="nav-icon"><AppIcon name="settings" :size="15" /></span>
        <span class="nav-label">设置</span>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";

const clipboardStore = useClipboardStore();

const props = defineProps<{
  activeCategory?: string;
  activeTag?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:activeCategory", value: string): void;
  (e: "update:activeTag", value: string | null): void;
  (e: "openSettings"): void;
  (e: "addTag"): void;
}>();

const categoryItems = computed(() => [
  { key: "all", icon: "clipboard" as AppIconName, label: "全部剪贴板", count: clipboardStore.filterCounts.all },
  { key: "text", icon: "type" as AppIconName, label: "文本", count: clipboardStore.filterCounts.text },
  { key: "image", icon: "image" as AppIconName, label: "图片", count: clipboardStore.filterCounts.image },
  { key: "file", icon: "file" as AppIconName, label: "文件", count: clipboardStore.filterCounts.file },
  { key: "link", icon: "link" as AppIconName, label: "链接", count: clipboardStore.filterCounts.link },
  { key: "code", icon: "code" as AppIconName, label: "代码", count: clipboardStore.filterCounts.code },
]);

const specialItems = computed(() => [
  { key: "favorites", icon: "star" as AppIconName, label: "收藏夹", count: clipboardStore.filterCounts.favorites },
  { key: "trash", icon: "trash" as AppIconName, label: "回收站", count: clipboardStore.trashCount },
]);

function selectCategory(key: string) {
  emit("update:activeCategory", key);
  emit("update:activeTag", null);
}

function selectTag(name: string) {
  // Only emit tag; parent updates category UI without calling setFilter
  // (setFilter clears activeTag).
  emit("update:activeTag", name);
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
  padding: 7px 8px;
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
  padding: 7px 8px;
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
  padding: 8px;
  margin-top: 4px;
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
}
</style>
