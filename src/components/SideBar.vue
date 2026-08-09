<template>
  <aside class="sidebar">
    <!-- Navigation: Categories (includes favorites as a view filter) -->
    <nav class="sidebar-section" :aria-label="$t('sidebar.categories')">
      <div class="sidebar-label">{{ $t('sidebar.categories') }}</div>
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
        :data-tooltip="isNarrow ? item.label : undefined"
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
    <nav class="sidebar-section" :aria-label="$t('sidebar.trash')">
      <button
        type="button"
        class="nav-item"
        :class="{ active: props.activeCategory === 'trash' }"
        :aria-current="props.activeCategory === 'trash' ? 'page' : undefined"
        :data-tooltip="isNarrow ? $t('sidebar.trash') : undefined"
        :aria-label="$t('sidebar.trash')"
        @click="selectCategory('trash')"
      >
        <span class="nav-icon"><AppIcon name="trash" :size="15" /></span>
        <span class="nav-label">{{ $t('sidebar.trash') }}</span>
        <span class="nav-count">{{ clipboardStore.trashCount }}</span>
      </button>
    </nav>

    <!-- Tags Section -->
    <div v-if="tagsEnabled" class="sidebar-section sidebar-tags-section">
      <div class="sidebar-tags-header">
        <div class="sidebar-label">{{ $t('sidebar.tagManagement') }}</div>
        <button
          type="button"
          class="tag-add"
          :data-tooltip="isNarrow ? $t('sidebar.newTag') : undefined"
          :title="$t('sidebar.newTag')"
          :aria-label="$t('sidebar.newTag')"
          @click="$emit('addTag')"
        >
          <AppIcon name="plus" :size="11" />
        </button>
      </div>
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
            :title="$t('sidebar.autoTagCreated')"
            aria-hidden="true"
          ><AppIcon name="sparkles" :size="11" /></span>
          <span class="tag-count">{{ tag.count }}</span>
        </button>

        <template v-if="moreTags.length > 0">
          <button
            type="button"
            class="tag-more-toggle"
            :aria-expanded="moreTagsOpen"
            :aria-label="moreTagsOpen ? $t('sidebar.collapse') : $t('sidebar.more')"
            @click="moreTagsOpen = !moreTagsOpen"
          >
            <span class="tag-more-label">{{ moreTagsOpen ? $t('sidebar.collapse') : $t('sidebar.more') }}</span>
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
                :title="$t('sidebar.autoTagCreated')"
                aria-hidden="true"
              ><AppIcon name="sparkles" :size="11" /></span>
              <span class="tag-count">{{ tag.count }}</span>
            </button>
          </template>
        </template>
      </div>
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
        :aria-label="$t('sidebar.quickMenu')"
        :title="$t('sidebar.quickMenu')"
        @click="toggleQuickMenu"
      >
        <AppIcon name="zap" :size="15" />
      </button>
      <button
        type="button"
        class="sidebar-icon-btn"
        :aria-label="$t('sidebar.settings')"
        :title="$t('sidebar.settings')"
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
import { useClipboardStore } from "../stores/clipboard";
import { useFeature } from "../composables/useFeature";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu.vue";
import type { Tag } from "../types";
import { useI18n } from "vue-i18n";
import { useSidebarMenus } from "../composables/useSidebarMenus";
import { useMediaQuery } from "../composables/useMediaQuery";
import { FILTER_DEFINITIONS } from "../utils/filterDefinitions";

const tagsEnabled = useFeature("tags");

/** Icon-rail layout hides nav labels via CSS; keep tooltips only there. */
const isNarrow = useMediaQuery("(max-width: 720px)");

const clipboardStore = useClipboardStore();
const { t } = useI18n();

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
  return tag.is_auto ? `${tag.name}（${t('sidebar.autoTagCreated')}）` : tag.name;
}

function tagAriaLabel(tag: Tag): string {
  return tag.is_auto ? `${tag.name}，${t('sidebar.autoTagCreated')}` : tag.name;
}

const tagMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  tag: null as Tag | null,
});

const tagMenuItems = computed<ContextMenuItem[]>(() => [
  { id: "edit", label: t('sidebar.editTag'), icon: "edit" },
  { id: "delete", label: t('sidebar.deleteTag'), icon: "trash", danger: true, separatorBefore: true },
]);

const categoryItems = computed(() =>
  FILTER_DEFINITIONS.map((definition) => ({
    ...definition,
    icon: definition.icon as AppIconName,
    label: t(definition.labelKey),
    count: clipboardStore.filterCounts[definition.key],
  })),
);

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

const {
  quickMenu,
  quickMenuAnchorEl,
  quickMenuItems,
  toggleQuickMenu,
  closeQuickMenu,
  onQuickMenuSelect,
} = useSidebarMenus((section) => emit("openSettings", section));

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
  padding: var(--space-3) var(--space-3) var(--space-1);
  flex-shrink: 0;
}

.sidebar-tags-section {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-tags-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-1);
  flex-shrink: 0;
  margin-bottom: var(--space-2);
}

/* The label's bottom padding serves as spacing below the title; inside the
   header it would offset the title from the add button, so drop it there.
   Keep the left padding so the title aligns with the other section labels. */
.sidebar-tags-header .sidebar-label {
  padding: 0 0 0 var(--space-2);
}

.sidebar-label {
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-tertiary);
  padding: 0 var(--space-2) var(--space-2);
}

.nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 7px var(--space-2);
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-secondary);
  font-size: var(--text-base);
  transition: background var(--transition-fast), color var(--transition-fast);
}

.nav-item:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent-text);
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
  font-size: var(--text-xs);
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
  /* Extend to the sidebar edges so the scrollbar hugs the right border, then
     restore the horizontal inset with matching padding. */
  margin: 0 calc(-1 * var(--space-3));
  padding: 0 var(--space-3);
}

.tag-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 7px var(--space-2);
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--text-md);
  color: var(--text-secondary);
  transition: background var(--transition-fast);
}

.tag-item:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.tag-item.active {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.tag-dot {
  width: 7px;
  height: 7px;
  border-radius: var(--radius-pill);
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
  color: var(--accent-text);
  opacity: 0.9;
}

.tag-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.tag-more-toggle {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  margin-top: 2px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  transition: background var(--transition-fast), color var(--transition-fast);
}

.tag-more-toggle:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
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
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.tag-add:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.sidebar-bottom {
  padding: var(--space-2) var(--space-3) var(--space-3);
  margin-top: auto;
  flex-shrink: 0;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-1);
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
  background: var(--accent-soft);
  color: var(--accent-text);
}

/* Left (quick-menu) button fills remaining width for a larger click target */
.sidebar-icon-btn-grow {
  flex: 1;
  justify-content: flex-start;
  padding-left: var(--space-3);
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
    padding: var(--space-2);
  }

  .sidebar-tags-header {
    justify-content: center;
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
