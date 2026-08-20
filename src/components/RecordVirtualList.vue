<!-- Windowed record list: renders only rows near the viewport (see
     useVirtualList) plus the load-more footer. Scroll element, ARIA
     activedescendant, and row actions all stay wired to the parent
     (RecordList) via the forwarded ref / re-emitted events.

     Pinned rows sit in-flow at the top of the scroller. When that block
     leaves the viewport, a capped overlay (`.pinned-dock`) shows the same
     block until the in-flow copy intersects again. -->
<template>
  <div class="record-list-host">
    <div
      v-if="docked"
      class="pinned-dock"
    >
      <PinnedRecordsBlock
        v-if="pinnedRecords.length > 0"
        :records="pinnedRecords"
        :collapsed="clipboardStore.pinnedCollapsed"
        :layout="layout"
        :grid-cols="gridCols"
        :option-id-prefix="DOCK_OPTION_PREFIX"
        :interactive="docked"
        :batch-mode="clipboardStore.batchMode"
        :selected-ids="clipboardStore.selectedIds"
        :selected-id="clipboardStore.selectedId"
        :trash-filter="clipboardStore.trashFilter"
        :last-incoming-id="clipboardStore.lastIncomingId"
        :leaving-ids="leavingIds"
        :search-query="clipboardStore.searchQuery"
        :source-overrides="sourceOverrides"
        :is-pinned="isPinned"
        :is-option-tabbable="isOptionTabbable"
        @toggle-collapsed="clipboardStore.togglePinnedCollapsed()"
        @item-click="(id, event) => emit('item-click', id, event)"
        @item-activate="emit('item-activate', $event)"
        @item-context-menu="(e, r) => emit('item-context-menu', e, r)"
        @item-paste="emit('item-paste', $event)"
        @item-favorite="emit('item-favorite', $event)"
        @item-toggle-pin="emit('item-toggle-pin', $event)"
        @item-delete="emit('item-delete', $event)"
        @item-restore="emit('item-restore', $event)"
      />
    </div>
    <div
      class="record-list"
      :class="{
        'view-grid': layout === 'grid',
        reloading: reloading,
        'view-fade-a': fadeArmed && !fadeOn,
        'view-fade-b': fadeArmed && fadeOn,
      }"
      :style="layout === 'grid' ? { gridTemplateColumns: `repeat(${gridCols}, minmax(0, 1fr))` } : undefined"
      :ref="bindScrollEl"
      role="listbox"
      :aria-label="$t('record.clipboardRecords')"
      :aria-activedescendant="activeDescendantId"
      :aria-busy="reloading"
      tabindex="-1"
      @scroll="emit('scroll')"
    >
      <div
        v-if="pinnedRecords.length > 0"
        class="pinned-block"
        :class="{ 'is-docked': docked }"
        :inert="docked"
        :ref="bindPinnedBlock"
      >
        <PinnedRecordsBlock
          :records="pinnedRecords"
          :collapsed="clipboardStore.pinnedCollapsed"
          :layout="layout"
          :grid-cols="gridCols"
          :option-id-prefix="RECORD_OPTION_PREFIX"
          :interactive="!docked"
          :batch-mode="clipboardStore.batchMode"
          :selected-ids="clipboardStore.selectedIds"
          :selected-id="clipboardStore.selectedId"
          :trash-filter="clipboardStore.trashFilter"
          :last-incoming-id="clipboardStore.lastIncomingId"
          :leaving-ids="leavingIds"
          :search-query="clipboardStore.searchQuery"
          :source-overrides="sourceOverrides"
          :is-pinned="isPinned"
          :is-option-tabbable="isOptionTabbable"
          @toggle-collapsed="clipboardStore.togglePinnedCollapsed()"
          @item-click="(id, event) => emit('item-click', id, event)"
          @item-activate="emit('item-activate', $event)"
          @item-context-menu="(e, r) => emit('item-context-menu', e, r)"
          @item-paste="emit('item-paste', $event)"
          @item-favorite="emit('item-favorite', $event)"
          @item-toggle-pin="emit('item-toggle-pin', $event)"
          @item-delete="emit('item-delete', $event)"
          @item-restore="emit('item-restore', $event)"
        />
      </div>
      <div
        class="virtual-spacer"
        :class="{ 'grid-span': layout === 'grid' }"
        :style="{ height: `${padTop}px` }"
        aria-hidden="true"
      />
      <template v-for="item in displayItems" :key="item.key">
        <div
          v-if="item.type === 'divider'"
          class="pin-section-divider"
          :style="{ height: `${item.height}px` }"
          aria-hidden="true"
        />
        <RecordListItem
          v-else-if="item.type === 'record'"
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
  </div>
</template>

<script setup lang="ts">
import {
  computed,
  onUnmounted,
  ref,
  watch,
  type ComponentPublicInstance,
  type VNodeRef,
} from "vue";
import { useClipboardStore } from "../stores/clipboard";
import PinnedRecordsBlock from "./PinnedRecordsBlock.vue";
import RecordListItem from "./RecordListItem.vue";
import type { ListLayout, WindowItem } from "../composables/useVirtualList";
import type { ClipboardRecord } from "../types";
import { DOCK_OPTION_PREFIX, RECORD_OPTION_PREFIX } from "../utils/pinnedList";

const props = defineProps<{
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
  /** Reports the in-flow pinned block so the virtualizer can offset unpinned rows. */
  setPinnedBlockEl: (el: unknown) => void;
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
  docked: [value: boolean];
}>();

const clipboardStore = useClipboardStore();

const pinnedRecords = computed(() =>
  clipboardStore.filteredRecords.filter((r) => r.is_pinned),
);

const docked = ref(false);
const scrollRoot = ref<HTMLElement | null>(null);
const pinnedBlockRef = ref<HTMLElement | null>(null);
let pinObserver: IntersectionObserver | null = null;

function bindScrollEl(el: Element | ComponentPublicInstance | null) {
  scrollRoot.value = el instanceof HTMLElement ? el : null;
  const se = props.scrollEl;
  if (typeof se === "function") se(el, {});
}

function bindPinnedBlock(el: unknown) {
  pinnedBlockRef.value = (el as HTMLElement | null) ?? null;
  props.setPinnedBlockEl(el);
}

function setDocked(value: boolean) {
  if (docked.value === value) return;
  docked.value = value;
  emit("docked", value);
}

function syncPinObserver() {
  pinObserver?.disconnect();
  pinObserver = null;
  const root = scrollRoot.value;
  const target = pinnedBlockRef.value;
  if (!root || !target || pinnedRecords.value.length === 0) {
    setDocked(false);
    return;
  }
  pinObserver = new IntersectionObserver(
    ([entry]) => {
      setDocked(pinnedRecords.value.length > 0 && !entry.isIntersecting);
    },
    { root, threshold: 0 },
  );
  pinObserver.observe(target);
}

watch(
  [scrollRoot, pinnedBlockRef, () => pinnedRecords.value.length],
  () => {
    syncPinObserver();
  },
  { flush: "post" },
);

onUnmounted(() => {
  pinObserver?.disconnect();
  pinObserver = null;
});
</script>

<style scoped>
.record-list-host {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.pinned-dock {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 4;
  max-height: 40%;
  overflow-y: auto;
  background: var(--bg-surface);
  box-shadow: var(--shadow-md);
}

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

.view-grid .pinned-block {
  grid-column: 1 / -1;
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

.pinned-block.is-docked {
  visibility: hidden;
  pointer-events: none;
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
