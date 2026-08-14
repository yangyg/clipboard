<template>
  <div class="batch-bar">
    <div class="batch-info">
      <AppIcon name="batch" :size="13" />
      {{ $t('batch.selected', { count: clipboardStore.selectedIds.size }) }}
    </div>
    <div class="batch-actions">
      <button
        type="button"
        class="batch-btn"
        :disabled="!hasRows"
        :title="allVisibleSelected ? $t('batch.deselectAll') : $t('batch.selectAll')"
        @click="toggleSelectAll"
      >
        <AppIcon name="batch" :size="13" />
        {{ allVisibleSelected ? $t('batch.deselectAll') : $t('batch.selectAll') }}
      </button>
      <template v-if="clipboardStore.trashFilter">
        <button type="button" class="batch-btn" :disabled="!hasSelection" @click="batchRestore">
          <AppIcon name="restore" :size="13" /> {{ $t('common.restore') }}
        </button>
        <button type="button" class="batch-btn danger" :disabled="!hasSelection" @click="batchDelete">
          <AppIcon name="trash" :size="13" /> {{ $t('record.permanentDelete') }}
        </button>
      </template>
      <template v-else>
        <button type="button" class="batch-btn" :disabled="!hasSelection" @click="batchCopy">
          <AppIcon name="copy" :size="13" /> {{ $t('batch.copy') }}
        </button>
        <button type="button" class="batch-btn" :disabled="!hasSelection" @click="batchFavorite">
          <AppIcon name="star" :size="13" /> {{ $t('batch.favorite') }}
        </button>
        <button type="button" class="batch-btn danger" :disabled="!hasSelection" @click="batchDelete">
          <AppIcon name="trash" :size="13" /> {{ $t('batch.delete') }}
        </button>
      </template>
      <button type="button" class="batch-btn batch-btn-icon" :title="$t('batch.exit')" :aria-label="$t('batch.exit')" @click="toggleBatchMode">
        <AppIcon name="close" :size="13" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import AppIcon from "./icons/AppIcon.vue";
import { useClipboardStore } from "../stores/clipboard";
import { useBatchActions } from "../composables/useBatchActions";

const clipboardStore = useClipboardStore();
const { toggleBatchMode, batchCopy, batchFavorite, batchDelete, batchRestore } = useBatchActions();

const hasSelection = computed(() => clipboardStore.selectedIds.size > 0);
const hasRows = computed(() => clipboardStore.filteredRecords.length > 0);
const allVisibleSelected = computed(() => {
  const list = clipboardStore.filteredRecords;
  return list.length > 0 && list.every((r) => clipboardStore.selectedIds.has(r.id));
});

function toggleSelectAll() {
  if (allVisibleSelected.value) clipboardStore.clearBatchSelection();
  else clipboardStore.selectAllFiltered();
}
</script>

<style scoped>
.batch-bar {
  padding: var(--space-2) var(--space-4);
  background: var(--accent-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, transparent);
  box-shadow: var(--shadow-sm);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.batch-info {
  font-size: var(--text-sm);
  color: var(--accent-text);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.batch-actions {
  display: flex;
  gap: var(--space-2);
}

.batch-btn {
  height: var(--btn-height-sm);
  padding: 0 var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: var(--space-1);
  background: var(--bg-surface);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  transition:
    background var(--transition-fast),
    color var(--transition-fast),
    border-color var(--transition-fast),
    filter var(--transition-fast),
    transform var(--transition-fast);
  cursor: pointer;
  font-family: inherit;
}

.batch-btn:hover:not(:disabled) {
  background: var(--accent-softer);
  color: var(--accent-text);
  border-color: color-mix(in srgb, var(--accent) 36%, var(--border-default));
}

/* Pressed feedback mirrors the global .btn:active convention */
.batch-btn:active:not(:disabled) {
  transform: scale(0.97);
  filter: brightness(0.94);
}

.batch-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Square variant for the icon-only exit button */
.batch-btn-icon {
  width: var(--btn-height-sm);
  padding: 0;
  justify-content: center;
}

.batch-btn.danger {
  background: var(--danger-soft);
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 20%, transparent);
}

.batch-btn.danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--danger) 20%, transparent);
  border-color: color-mix(in srgb, var(--danger) 45%, transparent);
}

.batch-btn.danger:active:not(:disabled) {
  background: color-mix(in srgb, var(--danger) 24%, transparent);
  border-color: color-mix(in srgb, var(--danger) 60%, transparent);
}

.batch-btn.danger:focus-visible {
  outline: 2px solid var(--danger);
  outline-offset: 2px;
}
</style>
