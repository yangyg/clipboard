<!-- Windowed record list: renders only rows near the viewport (see
     useVirtualList) plus the load-more footer. Scroll element, ARIA
     activedescendant, and row actions all stay wired to the parent
     (RecordList) via the forwarded ref / re-emitted events. -->
<template>
  <div
    class="record-list"
    :class="{
      'view-grid': layout === 'grid',
      reloading: reloading,
      'view-fade-a': fadeArmed && !fadeOn,
      'view-fade-b': fadeArmed && fadeOn,
    }"
    :style="layout === 'grid' ? { gridTemplateColumns: `repeat(${gridCols}, minmax(0, 1fr))` } : undefined"
    :ref="scrollEl"
    role="listbox"
    :aria-label="$t('record.clipboardRecords')"
    :aria-activedescendant="activeDescendantId"
    :aria-busy="reloading"
    tabindex="-1"
    @scroll="emit('scroll')"
  >
    <div
      class="virtual-spacer"
      :class="{ 'grid-span': layout === 'grid' }"
      :style="{ height: `${padTop}px` }"
      aria-hidden="true"
    />
    <template v-for="item in displayItems" :key="item.key">
      <button
        v-if="item.type === 'label'"
        type="button"
        class="section-label"
        :aria-expanded="!clipboardStore.pinnedCollapsed"
        :aria-label="clipboardStore.pinnedCollapsed ? $t('record.expandPinned') : $t('record.collapsePinned')"
        :title="clipboardStore.pinnedCollapsed ? $t('record.expandPinned') : $t('record.collapsePinned')"
        @click.stop="clipboardStore.togglePinnedCollapsed()"
      >
        <AppIcon name="pin" :size="11" />
        <span>{{ $t('record.pinnedSection') }}</span>
        <span class="section-label-count">{{ pinnedCount }}</span>
        <span
          class="section-label-chevron"
          :class="{ collapsed: clipboardStore.pinnedCollapsed }"
          aria-hidden="true"
        />
      </button>
      <div
        v-else-if="item.type === 'divider'"
        class="pin-section-divider"
        :style="{ height: `${item.height}px` }"
        aria-hidden="true"
      />
      <RecordListItem
        v-else
        :record="item.record!"
        :thumb="item.thumb"
        :batch-mode="clipboardStore.batchMode"
        :checked="clipboardStore.selectedIds.has(item.record!.id)"
        :selected="clipboardStore.selectedId === item.record!.id"
        :tabbable="isOptionTabbable(item.record!.id)"
        :trash-filter="clipboardStore.trashFilter"
        :pinned="isPinned(item.record!)"
        :is-new="item.record!.id === clipboardStore.lastIncomingId"
        :is-leaving="leavingIds.has(item.record!.id)"
        :search-query="clipboardStore.searchQuery"
        :source-overrides="sourceOverrides"
        :measure-row="measureRow"
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
    <div
      class="virtual-spacer"
      :class="{ 'grid-span': layout === 'grid' }"
      :style="{ height: `${padBottom}px` }"
      aria-hidden="true"
    />

    <!-- Footer: load-more status only -->
    <div v-if="clipboardStore.isLoadingMore || clipboardStore.hasMore" class="list-footer">
      <span v-if="clipboardStore.isLoadingMore" class="footer-loading">
        <span class="loading-spinner small" aria-hidden="true"></span>{{ $t('common.loadMore') }}
      </span>
      <span v-else>{{ $t('common.scrollForMore') }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, type VNodeRef } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import AppIcon from "./icons/AppIcon.vue";
import RecordListItem from "./RecordListItem.vue";
import type { ListLayout, WindowItem } from "../composables/useVirtualList";
import type { ClipboardRecord } from "../types";

defineProps<{
  layout: ListLayout;
  gridCols: number;
  displayItems: WindowItem[];
  padTop: number;
  padBottom: number;
  reloading: boolean;
  /** Layout-switch fade toggles (see RecordList.setListLayout). */
  fadeArmed: boolean;
  fadeOn: boolean;
  /** Callback ref forwarding the scroll element to the parent —
   * useVirtualList / keyboard nav need direct element access. */
  scrollEl: VNodeRef;
  leavingIds: Set<number>;
  sourceOverrides: Record<string, string>;
  activeDescendantId: string | undefined;
  isPinned: (record: ClipboardRecord) => boolean;
  isOptionTabbable: (id: number) => boolean;
  /** Reports mounted/unmounted row elements to the virtualizer for measuring. */
  measureRow: (id: number, el: HTMLElement | null) => void;
}>();

const emit = defineEmits<{
  scroll: [];
  "item-click": [id: number, event?: MouseEvent];
  "item-activate": [id: number];
  "item-context-menu": [event: MouseEvent, record: ClipboardRecord];
  "item-paste": [id: number];
  "item-favorite": [id: number];
  "item-toggle-pin": [record: ClipboardRecord];
  "item-delete": [record: ClipboardRecord];
  "item-restore": [id: number];
}>();

const clipboardStore = useClipboardStore();

const pinnedCount = computed(
  () => clipboardStore.filteredRecords.filter((r) => r.is_pinned).length,
);
</script>

<style scoped>
.record-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-1) 0 var(--space-2);
}

.record-list.reloading {
  opacity: 0.72;
  transition: opacity var(--transition-fast);
}

/* Layout switch cross-fade (spec §3.5): a fresh one-way fade runs per toggle.
   Two identical keyframes alternate by class name so switching restarts the
   animation without remounting the scroll container (which would lose the
   scroll position). Pure CSS — never gates mounting, honors anim-disabled /
   prefers-reduced-motion. */
.view-fade-a {
  animation: view-fade-a var(--transition-normal) ease;
}
.view-fade-b {
  animation: view-fade-b var(--transition-normal) ease;
}
@keyframes view-fade-a {
  from { opacity: 0; }
  to { opacity: 1; }
}
@keyframes view-fade-b {
  from { opacity: 0; }
  to { opacity: 1; }
}
:global(body.anim-disabled) .view-fade-a,
:global(body.anim-disabled) .view-fade-b {
  animation: none;
}
@media (prefers-reduced-motion: reduce) {
  .view-fade-a,
  .view-fade-b {
    animation: none;
  }
}

/* —— Grid view: vertical cards (original structure) —— */
.record-list.view-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px; /* must match GRID_GAP in useVirtualList */
  padding: var(--space-3);
  align-content: start;
}

.view-grid .section-label {
  grid-column: 1 / -1;
  padding: var(--space-1) 2px 0;
}

.view-grid .pin-section-divider {
  grid-column: 1 / -1;
  margin-inline: 2px;
}

.view-grid .list-footer {
  grid-column: 1 / -1;
  margin-top: 0;
  border-top: none;
  padding: var(--space-1) 0 var(--space-2);
}

.virtual-spacer {
  width: 100%;
  flex-shrink: 0;
  pointer-events: none;
}

/* H-3: Grid spacer must span all columns to maintain scroll height */
.virtual-spacer.grid-span {
  grid-column: 1 / -1;
}

.section-label {
  width: 100%;
  box-sizing: border-box;
  font-size: var(--text-xs, 0.625rem);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--pin);
  padding: var(--space-3) var(--space-4) var(--space-1);
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border: none;
  background: transparent;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  border-radius: var(--radius-sm);
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

.pin-section-divider {
  box-sizing: border-box;
  flex-shrink: 0;
  width: 100%;
  margin: 0;
  padding: 0 var(--space-4);
  pointer-events: none;
  display: flex;
  align-items: center;
}

.pin-section-divider::after {
  content: "";
  display: block;
  width: 100%;
  height: 1px;
  background: var(--border-subtle);
}

/* Footer */
.list-footer {
  padding: var(--space-3) var(--space-4);
  text-align: center;
  font-size: var(--text-md);
  color: var(--text-muted, var(--text-tertiary));
  border-top: 1px solid var(--border-light, var(--border-subtle));
  margin-top: var(--space-1);
}

.footer-loading {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}
</style>
