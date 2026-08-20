<!-- In-flow / dock copy of the pinned header + rows. Parent decides
     interactivity (inert) and which option-id prefix to use. -->
<template>
  <div
    class="pinned-records-block"
    :class="{ 'is-grid': layout === 'grid' }"
    :style="layout === 'grid' ? { gridTemplateColumns: `repeat(${gridCols}, minmax(0, 1fr))` } : undefined"
  >
    <button
      type="button"
      class="section-label"
      :aria-expanded="!collapsed"
      :aria-label="collapsed ? $t('record.expandPinned') : $t('record.collapsePinned')"
      :title="collapsed ? $t('record.expandPinned') : $t('record.collapsePinned')"
      @click.stop="emit('toggle-collapsed')"
    >
      <AppIcon name="pin" :size="11" />
      <span>{{ $t('record.pinnedSection') }}</span>
      <span class="section-label-count">{{ records.length }}</span>
      <span
        class="section-label-chevron"
        :class="{ collapsed: collapsed }"
        aria-hidden="true"
      />
    </button>
    <template v-if="!collapsed">
      <RecordListItem
        v-for="record in records"
        :key="record.id"
        :record="record"
        :thumb="thumbSrc(record)"
        :option-id-prefix="optionIdPrefix"
        :batch-mode="batchMode"
        :checked="selectedIds.has(record.id)"
        :selected="selectedId === record.id"
        :tabbable="interactive && isOptionTabbable(record.id)"
        :trash-filter="trashFilter"
        :pinned="isPinned(record)"
        :is-new="record.id === lastIncomingId"
        :is-leaving="leavingIds.has(record.id)"
        :search-query="searchQuery"
        :source-overrides="sourceOverrides"
        @click="(id: number, event?: MouseEvent) => emit('item-click', id, event)"
        @activate="emit('item-activate', $event)"
        @context-menu="(e, r) => emit('item-context-menu', e, r)"
        @paste="emit('item-paste', $event)"
        @favorite="emit('item-favorite', $event)"
        @toggle-pin="emit('item-toggle-pin', $event)"
        @delete="emit('item-delete', $event)"
        @restore="emit('item-restore', $event)"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
import type { ListLayout } from "../composables/useVirtualList";
import type { ClipboardRecord } from "../types";
import { recordThumbSrc } from "../utils/mediaUrl";
import AppIcon from "./icons/AppIcon.vue";
import RecordListItem from "./RecordListItem.vue";

defineProps<{
  records: ClipboardRecord[];
  collapsed: boolean;
  layout: ListLayout;
  gridCols: number;
  optionIdPrefix: string;
  interactive: boolean;
  batchMode: boolean;
  selectedIds: Set<number>;
  selectedId: number | null;
  trashFilter: boolean;
  lastIncomingId: number | null;
  leavingIds: Set<number>;
  searchQuery: string;
  sourceOverrides: Record<string, string>;
  isPinned: (record: ClipboardRecord) => boolean;
  isOptionTabbable: (id: number) => boolean;
}>();

const emit = defineEmits<{
  "toggle-collapsed": [];
  "item-click": [id: number, event?: MouseEvent];
  "item-activate": [id: number];
  "item-context-menu": [event: MouseEvent, record: ClipboardRecord];
  "item-paste": [id: number];
  "item-favorite": [id: number];
  "item-toggle-pin": [record: ClipboardRecord];
  "item-delete": [record: ClipboardRecord];
  "item-restore": [id: number];
}>();

function thumbSrc(record: ClipboardRecord): string | null {
  return recordThumbSrc(record);
}
</script>

<style scoped>
.pinned-records-block.is-grid {
  display: grid;
  gap: 8px;
  align-content: start;
}

.section-label {
  width: 100%;
  box-sizing: border-box;
  grid-column: 1 / -1;
  font-size: var(--text-xs, 0.625rem);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--pin);
  padding: var(--space-2) var(--space-4);
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border: none;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.section-label:hover {
  background: var(--bg-hover);
}

.section-label:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.section-label-count {
  font-weight: 500;
  letter-spacing: 0;
  text-transform: none;
  opacity: 0.75;
}

.section-label-chevron {
  margin-left: auto;
  width: 0;
  height: 0;
  border-left: 3.5px solid transparent;
  border-right: 3.5px solid transparent;
  border-top: 4px solid currentColor;
  transition: transform var(--transition-fast);
}

.section-label-chevron.collapsed {
  transform: rotate(-90deg);
}

:global(body.anim-disabled) .section-label-chevron {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  .section-label-chevron {
    transition: none;
  }
}
</style>
