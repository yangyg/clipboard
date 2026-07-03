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
        <span class="nav-icon">{{ item.icon }}</span>
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
        <span class="nav-icon">{{ item.icon }}</span>
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
        <span>＋</span> 新建标签
      </div>
    </div>

    <!-- Bottom Actions -->
    <div class="sidebar-bottom">
      <div class="nav-item" @click="$emit('openSettings')">
        <span class="nav-icon">⚙</span>
        <span class="nav-label">设置</span>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useClipboardStore } from "../stores/clipboard";

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
  { key: "all", icon: "📋", label: "全部剪贴板", count: clipboardStore.filterCounts.all },
  { key: "text", icon: "📝", label: "文本", count: clipboardStore.filterCounts.text },
  { key: "image", icon: "🖼", label: "图片", count: clipboardStore.filterCounts.image },
  { key: "file", icon: "📄", label: "文件", count: clipboardStore.filterCounts.file },
  { key: "link", icon: "🔗", label: "链接", count: clipboardStore.filterCounts.link },
  { key: "code", icon: "</>", label: "代码", count: clipboardStore.filterCounts.code },
]);

const specialItems = computed(() => [
  { key: "favorites", icon: "⭐", label: "收藏夹", count: clipboardStore.filterCounts.favorites },
  { key: "trash", icon: "🗑", label: "回收站", count: clipboardStore.trashCount },
]);

function selectCategory(key: string) {
  emit("update:activeCategory", key);
  emit("update:activeTag", null);
}

function selectTag(name: string) {
  emit("update:activeTag", name);
}
</script>

<style scoped>
.sidebar {
  width: 200px;
  min-width: 160px;
  max-width: 260px;
  flex-shrink: 0;
  height: 100%;
  background: var(--bg-elevated);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  user-select: none;
}

.sidebar-header {
  height: 42px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.logo-icon {
  width: 22px;
  height: 22px;
  background: linear-gradient(135deg, var(--accent), var(--accent-light, #6b85fa));
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.logo-glyph {
  font-size: 11px;
  color: #fff;
}

.logo-text {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}

.logo-version {
  font-size: 10px;
  font-weight: 500;
  color: var(--text-tertiary);
  padding: 1px 6px;
  background: var(--bg-surface);
  border-radius: 10px;
  margin-left: auto;
}

.sidebar-section {
  padding: 6px 0;
}

.sidebar-label {
  font-size: 10.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted, var(--text-tertiary));
  padding: 6px 16px 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 16px;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 500;
  position: relative;
}

.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 3px;
  background: var(--accent);
  border-radius: 0 3px 3px 0;
}

.nav-icon {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  flex-shrink: 0;
}

.nav-label {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nav-count {
  margin-left: auto;
  font-size: 11.5px;
  font-weight: 600;
  background: var(--bg-surface);
  padding: 1px 7px;
  border-radius: 10px;
  color: var(--text-tertiary);
  min-width: 24px;
  text-align: center;
  flex-shrink: 0;
}

.nav-item.active .nav-count {
  background: rgba(79, 110, 247, 0.15);
  color: var(--accent);
}

/* Tags */
.sidebar-tags-section {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
  padding-bottom: 2px;
}

.tags-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.tag-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  cursor: pointer;
  transition: background var(--transition-fast);
  font-size: 12px;
  color: var(--text-secondary);
}

.tag-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.tag-item.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.tag-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tag-name {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tag-count {
  font-size: 10px;
  color: var(--text-muted, var(--text-tertiary));
  flex-shrink: 0;
}

.tag-add {
  padding: 6px 16px;
  color: var(--text-tertiary);
  font-size: 12.5px;
  cursor: pointer;
  transition: color var(--transition-fast);
  display: flex;
  align-items: center;
  gap: 6px;
}

.tag-add:hover {
  color: var(--accent);
}

/* Bottom */
.sidebar-bottom {
  padding: 8px 0 12px;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
}
</style>
