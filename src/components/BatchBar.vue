<template>
  <div class="batch-bar">
    <div class="batch-info">
      <AppIcon name="batch" :size="13" />
      {{ $t('batch.selected', { count: clipboardStore.selectedIds.size }) }}
    </div>
    <div class="batch-actions">
      <template v-if="clipboardStore.trashFilter">
        <button type="button" class="batch-btn" @click="batchRestore">
          <AppIcon name="restore" :size="13" /> {{ $t('common.restore') }}
        </button>
        <button type="button" class="batch-btn danger" @click="batchDelete">
          <AppIcon name="trash" :size="13" /> {{ $t('record.permanentDelete') }}
        </button>
      </template>
      <template v-else>
        <button type="button" class="batch-btn" @click="batchCopy">
          <AppIcon name="copy" :size="13" /> {{ $t('batch.copy') }}
        </button>
        <button type="button" class="batch-btn" @click="batchFavorite">
          <AppIcon name="star" :size="13" /> {{ $t('batch.favorite') }}
        </button>
        <button type="button" class="batch-btn danger" @click="batchDelete">
          <AppIcon name="trash" :size="13" /> {{ $t('batch.delete') }}
        </button>
      </template>
      <button type="button" class="batch-btn" :aria-label="$t('batch.exit')" @click="toggleBatchMode">
        <AppIcon name="close" :size="13" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "./icons/AppIcon.vue";
import { useClipboardStore } from "../stores/clipboard";
import { useBatchActions } from "../composables/useBatchActions";

const clipboardStore = useClipboardStore();
const { toggleBatchMode, batchCopy, batchFavorite, batchDelete, batchRestore } = useBatchActions();
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
    filter var(--transition-fast);
  cursor: pointer;
  font-family: inherit;
}

.batch-btn:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.batch-btn.danger {
  background: var(--danger-soft);
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 20%, transparent);
}

.batch-btn.danger:hover {
  filter: brightness(1.05);
}
</style>
