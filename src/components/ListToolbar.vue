<template>
  <div class="list-toolbar">
    <div class="list-toolbar-left">
      <span class="list-title">{{ categoryTitle }}</span>
      <span class="list-count">{{ listCountLabel }}</span>
    </div>
    <div class="list-toolbar-right">
      <button
        v-if="clipboardStore.trashFilter && clipboardStore.trashCount > 0"
        type="button"
        class="empty-trash-btn"
        @click="onEmptyTrash"
      >{{ $t('listView.emptyTrashBtn') }}</button>
      <select
        class="list-sort"
        :value="clipboardStore.listSort"
        :title="$t('sort.listSort')"
        :aria-label="$t('sort.listSort')"
        @change="onSortChange"
      >
        <option
          v-for="opt in LIST_SORT_OPTIONS"
          :key="opt.value"
          :value="opt.value"
        >{{ $t(opt.labelKey) }}</option>
      </select>
      <div class="view-toggle" role="group" :aria-label="$t('listView.listView')">
        <button
          type="button"
          class="view-toggle-btn"
          :class="{ active: listLayout === 'list' }"
          :title="$t('listView.listView')"
          :aria-label="$t('listView.listView')"
          :aria-pressed="listLayout === 'list'"
          @click="emit('set-layout', 'list')"
        ><AppIcon name="list" :size="14" /></button>
        <button
          type="button"
          class="view-toggle-btn"
          :class="{ active: listLayout === 'grid' }"
          :title="$t('listView.gridView')"
          :aria-label="$t('listView.gridView')"
          :aria-pressed="listLayout === 'grid'"
          @click="emit('set-layout', 'grid')"
        ><AppIcon name="grid" :size="14" /></button>
      </div>
      <button
        type="button"
        class="list-tool-btn"
        :class="{ active: clipboardStore.batchMode }"
        :title="$t('panel.batch')"
        :aria-label="$t('panel.batch')"
        :aria-pressed="clipboardStore.batchMode"
        @click="toggleBatchMode"
      ><AppIcon name="batch" :size="14" /></button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useClipboardStore, LIST_SORT_OPTIONS, type ListSort } from "../stores/clipboard";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useBatchActions } from "../composables/useBatchActions";
import type { ListLayout } from "../composables/useVirtualList";
import AppIcon from "./icons/AppIcon.vue";

defineProps<{
  listLayout: ListLayout;
}>();

const emit = defineEmits<{
  "set-layout": [mode: ListLayout];
}>();

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode } = useBatchActions();
const { t } = useI18n();

const CATEGORY_TITLE_KEYS: Record<string, string> = {
  all: "category.all",
  text: "category.text",
  image: "category.image",
  file: "category.file",
  link: "category.link",
  code: "category.code",
  favorites: "category.favorites",
  trash: "category.trash",
};

const categoryTitle = computed(() => {
  if (clipboardStore.trashFilter) return t('category.trash');
  const typeKey = clipboardStore.activeFilter;
  const typePart =
    typeKey !== "all" ? t(CATEGORY_TITLE_KEYS[typeKey] ?? typeKey) : null;
  const tagPart = clipboardStore.activeTag;
  if (typePart && tagPart) return `${typePart} · ${tagPart}`;
  if (tagPart) return tagPart;
  if (typePart) return typePart;
  return t('category.all');
});

const listCountLabel = computed(() => {
  if (clipboardStore.searchQuery) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? t('record.countFound', { n }) : t('record.countTotal', { n });
  }
  if (clipboardStore.trashFilter) {
    return t('record.countTotal', { n: clipboardStore.trashCount });
  }
  if (clipboardStore.activeTag) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? t('record.countLoaded', { n }) : t('record.countTotal', { n });
  }
  if (clipboardStore.activeFilter === "favorites") {
    return t('record.countTotal', { n: clipboardStore.filterCounts.favorites });
  }
  if (clipboardStore.activeFilter !== "all") {
    return t('record.countTotal', { n: clipboardStore.filterCounts[clipboardStore.activeFilter] });
  }
  return t('record.countTotal', { n: clipboardStore.filterCounts.all });
});

function onSortChange(e: Event) {
  const value = (e.target as HTMLSelectElement).value as ListSort;
  clipboardStore.setListSort(value);
}

async function onEmptyTrash() {
  const ok = await confirm({
    title: t('confirm.emptyTrashTitle'),
    message: t('confirm.emptyTrashMsg'),
    confirmText: t('confirm.emptyTrashConfirm'),
    danger: true,
  });
  if (ok) {
    try {
      await clipboardStore.emptyTrash();
      toast(t('confirm.trashEmptied'), "success");
    } catch {
      toast(t('confirm.emptyFailed'), "error");
    }
  }
}
</script>

<style scoped>
.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  height: 44px;
  padding: 0 var(--space-3);
  flex-shrink: 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border-default) 60%, transparent);
}

.list-toolbar-left {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.list-title {
  font-size: var(--text-sm, 0.6875rem);
  font-weight: 600;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 7rem;
}

.list-count {
  font-size: var(--text-sm, 0.6875rem);
  font-weight: 500;
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.list-toolbar-right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
  margin-left: auto;
}

.empty-trash-btn {
  height: var(--btn-height-sm);
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs, 0.625rem);
  font-weight: 500;
  background: var(--danger-soft);
  color: var(--danger);
  border: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  cursor: pointer;
  transition: background var(--transition-fast);
  font-family: inherit;
}

.empty-trash-btn:hover {
  background: color-mix(in srgb, var(--danger) 20%, transparent);
}

.list-sort {
  height: var(--btn-height-sm);
  max-width: 7rem;
  padding: 0 var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
  color: var(--text-secondary);
  font-size: var(--text-sm, 0.6875rem);
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

.list-tool-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  color: var(--text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
}

.list-tool-btn:hover,
.list-tool-btn.active {
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  color: var(--accent-text);
}

.view-toggle {
  display: flex;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--bg-surface);
}

.view-toggle-btn {
  width: 28px;
  height: var(--btn-height-sm);
  border: none;
  background: transparent;
  color: var(--text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.view-toggle-btn + .view-toggle-btn {
  border-left: 1px solid var(--border-subtle);
}

.view-toggle-btn:hover {
  color: var(--text-secondary);
  background: var(--bg-hover);
}

.view-toggle-btn.active {
  color: var(--accent-text);
  background: var(--accent-soft);
}

.view-toggle-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
  z-index: 1;
}
</style>
