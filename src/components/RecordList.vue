<template>
  <div ref="wrapperRef" class="record-list-wrapper">
    <div
      ref="listColRef"
      class="list-column"
      :class="{ 'list-column--full': usePreviewDrawer }"
      :style="
        usePreviewDrawer
          ? undefined
          : { width: listColWidth + 'px', minWidth: listColWidth + 'px', maxWidth: 'none', flex: 'none' }
      "
    >
      <!-- Middle-column chrome (window mode): matches design list toolbar -->
      <template v-if="showListChrome">
        <div class="list-chrome">
          <ListToolbar :list-layout="listLayout" @set-layout="setListLayout" />

          <Transition name="batch-bar">
            <div v-if="clipboardStore.batchMode" ref="batchBarRef" class="batch-bar-holder">
              <BatchBar />
            </div>
          </Transition>
        </div>
      </template>

      <!-- Loading / Empty → Record List.
           NOTE: deliberately NOT wrapped in a JS <Transition mode="out-in">: WebView2
           drops requestAnimationFrame callbacks while its host window is hidden, which
           stalls an out-in transition forever and leaves the list unmounted (blank list
           on cold start). The fade-in is a pure CSS animation on .list-body instead —
           CSS animations resume/complete on their own and never gate mounting. -->
      <ListEmptyState v-if="isEmptyOrLoading" />

      <!-- Record List (windowed: only mount rows near the viewport) -->
      <div
        v-else
        class="list-body list-body--enter"
        :style="{ paddingTop: clipboardStore.batchMode ? batchBarHeight + 'px' : 0 }"
      >
        <div
          v-if="isListReloading"
          class="list-reload-bar"
          role="status"
          :aria-label="$t('common.loading')"
        >
          <span class="loading-spinner small"></span>
          <span>{{ $t('common.loading') }}</span>
        </div>
        <div
          class="record-list"
          :class="{
            'view-grid': listLayout === 'grid',
            reloading: isListReloading,
            'view-fade-a': layoutFadeArmed && !layoutFadeOn,
            'view-fade-b': layoutFadeArmed && layoutFadeOn,
          }"
          :style="listLayout === 'grid' ? { gridTemplateColumns: `repeat(${gridCols}, minmax(0, 1fr))` } : undefined"
          ref="listRef"
          role="listbox"
          :aria-label="$t('record.clipboardRecords')"
          :aria-activedescendant="activeDescendantId"
          :aria-busy="isListReloading"
          tabindex="-1"
          @scroll="onListScroll"
        >
      <div
        class="virtual-spacer"
        :class="{ 'grid-span': listLayout === 'grid' }"
        :style="{ height: `${virtualPadTop}px` }"
        aria-hidden="true"
      />
      <template v-for="item in displayItems" :key="item.key">
        <div v-if="item.type === 'label'" class="section-label" aria-hidden="true"><AppIcon name="pin" :size="11" /> {{ $t('record.pinnedSection') }}</div>
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
          @click="onItemClick"
          @activate="onItemActivate"
          @context-menu="showContextMenu"
          @paste="quickPaste"
          @favorite="onRowFavorite"
          @toggle-pin="scheduleTogglePin"
          @delete="quickDelete"
          @restore="onRowRestore"
        />
      </template>
      <div
        class="virtual-spacer"
        :class="{ 'grid-span': listLayout === 'grid' }"
        :style="{ height: `${virtualPadBottom}px` }"
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

      <!-- Back to top: floats over the list column, scrolls only the list area -->
      <Transition name="back-top">
        <button
          v-if="showBackToTop"
          type="button"
          class="back-to-top-btn"
          :aria-label="$t('common.backToTop')"
          :title="$t('common.backToTop')"
          @click="scrollToTop"
        >
          <AppIcon name="arrowUp" :size="15" />
        </button>
      </Transition>
    </div>

    <!-- Resizer between list and preview (side-by-side only) -->
    <div
      v-if="previewVisible && !usePreviewDrawer"
      class="resizer"
      :class="{ active: listColDragging }"
      role="separator"
      aria-orientation="vertical"
      :aria-valuenow="listColWidth"
      :aria-valuemin="280"
      :aria-valuemax="720"
      tabindex="0"
      :aria-label="$t('record.resizeList')"
      @pointerdown="startListColResize"
      @keydown="onListColResizeKey"
    />

    <!-- Preview Pane: side-by-side (wide) or overlay drawer (tight host) -->
    <div
      v-if="previewVisible && usePreviewDrawer"
      class="preview-drawer-backdrop"
      @click="clipboardStore.clearSelection()"
    />
    <div
      v-if="previewVisible"
      class="preview-host"
      :class="{ 'preview-host--drawer': usePreviewDrawer }"
    >
      <PreviewPane :drawer="usePreviewDrawer" />
    </div>

    <!-- Context Menu -->
    <ContextMenu
      :visible="contextMenu.visible"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :items="contextMenuItems"
      @close="closeContextMenu"
      @select="onContextSelect"
    />

    <AliasDialog
      :visible="aliasDialog.visible"
      :record-id="aliasDialog.recordId"
      :initial-alias="aliasDialog.initialAlias"
      @close="closeAliasDialog"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import PreviewPane from "./PreviewPane.vue";
import ContextMenu from "./ContextMenu.vue";
import AliasDialog from "./AliasDialog.vue";
import BatchBar from "./BatchBar.vue";
import AppIcon from "./icons/AppIcon.vue";
import RecordListItem from "./RecordListItem.vue";
import ListToolbar from "./ListToolbar.vue";
import ListEmptyState from "./ListEmptyState.vue";
import { useVirtualList, type ListLayout } from "../composables/useVirtualList";
import { useColumnResize } from "../composables/useColumnResize";
import { useBatchBarHeight } from "../composables/useBatchBarHeight";
import { useRecordActions } from "../composables/useRecordActions";
import { buildSourceOverrides } from "../utils/sourceBadge";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const listRef = ref<HTMLElement | null>(null);

const sourceOverrides = computed(() =>
  buildSourceOverrides(settingsStore.settings.source_name_overrides),
);

// --- Floating batch bar (window mode): reserve its height as list padding ---
const batchBarRef = ref<HTMLElement | null>(null);
const { height: batchBarHeight } = useBatchBarHeight(batchBarRef);

// --- List / Preview column resize ---
const previewVisible = computed(
  () => !!clipboardStore.selectedRecord && !clipboardStore.batchMode
);
const {
  width: listColWidth,
  isDragging: listColDragging,
  isDefault: listColIsDefault,
  startResize: startListColResize,
  setWidth: setListColWidth,
} = useColumnResize({
  storageKey: "clipboard-list-col-width",
  defaultWidth: 400,
  min: 280,
  max: 720,
});
const listColRef = ref<HTMLElement | null>(null);

// On first run (no stored width), capture the list column's natural flex
// width so the fixed-width mode matches what the user already sees.
onMounted(() => {
  if (listColIsDefault.value && listColRef.value) {
    setListColWidth(listColRef.value.offsetWidth);
  }
  if (wrapperRef.value) {
    wrapperWidth.value = wrapperRef.value.clientWidth;
    wrapperRo = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w != null) wrapperWidth.value = w;
    });
    wrapperRo.observe(wrapperRef.value);
  }
});
onUnmounted(() => {
  wrapperRo?.disconnect();
  wrapperRo = null;
});

const LAYOUT_KEY = "clipvault-list-layout";
/** Alternate between two identical fade keyframes so each layout switch
 * restarts the enter animation without remounting the scroll container
 * (a remount would reset scrollTop). Armed only after the first switch so
 * the initial mount keeps its single .list-body--enter fade. */
const layoutFadeArmed = ref(false);
const layoutFadeOn = ref(false);

function readStoredLayout(): ListLayout {
  try {
    const v = localStorage.getItem(LAYOUT_KEY);
    if (v === "grid" || v === "list") return v;
  } catch {
    /* ignore */
  }
  return "list";
}

const listLayout = ref<ListLayout>(readStoredLayout());

const {
  displayItems,
  virtualPadTop,
  virtualPadBottom,
  flatItems,
  gridCols,
  scrollTop,
  onListScroll,
  fillViewportIfNeeded,
} = useVirtualList(listRef, listLayout);

function setListLayout(mode: ListLayout) {
  if (listLayout.value === mode) return;
  listLayout.value = mode;
  layoutFadeArmed.value = true;
  layoutFadeOn.value = !layoutFadeOn.value;
  try {
    localStorage.setItem(LAYOUT_KEY, mode);
  } catch {
    /* ignore */
  }
  void nextTick(() => fillViewportIfNeeded());
}

/** Window mode: toolbar lives in the list column (not spanning the preview). */
const showListChrome = computed(() => settingsStore.settings.app_mode === "window");

/** Show the loading / empty-state panel instead of the virtualized list. */
const isEmptyOrLoading = computed(
  () =>
    (clipboardStore.isLoading && clipboardStore.records.length === 0) ||
    (clipboardStore.filteredRecords.length === 0 && !clipboardStore.isLoading)
);

/** Filter/sort reload with existing rows still on screen — show top bar, keep list. */
const isListReloading = computed(
  () => clipboardStore.isLoading && clipboardStore.records.length > 0
);

/** Host width too tight for list+preview side-by-side → drawer overlay. */
const PREVIEW_DRAWER_BREAKPOINT = 560;
const wrapperRef = ref<HTMLElement | null>(null);
const wrapperWidth = ref(0);
let wrapperRo: ResizeObserver | null = null;
const usePreviewDrawer = computed(
  () => previewVisible.value && wrapperWidth.value > 0 && wrapperWidth.value < PREVIEW_DRAWER_BREAKPOINT
);

const {
  leavingIds,
  isPinned,
  scheduleTogglePin,
  contextMenu,
  contextMenuItems,
  showContextMenu,
  closeContextMenu,
  onContextSelect,
  aliasDialog,
  closeAliasDialog,
  quickPaste,
  onRowFavorite,
  onRowRestore,
  quickDelete,
  onItemClick,
  onItemActivate,
  showBackToTop,
  scrollToTop,
  activeDescendantId,
  isOptionTabbable,
} = useRecordActions({
  listRef,
  scrollTop,
  flatItems: () => flatItems.value,
  isEmptyOrLoading: () => isEmptyOrLoading.value,
  selectedId: () => clipboardStore.selectedId,
});

function onListColResizeKey(e: KeyboardEvent) {
  const step = e.shiftKey ? 40 : 16;
  if (e.key === "ArrowLeft") {
    e.preventDefault();
    setListColWidth(listColWidth.value - step);
  } else if (e.key === "ArrowRight") {
    e.preventDefault();
    setListColWidth(listColWidth.value + step);
  } else if (e.key === "Home") {
    e.preventDefault();
    setListColWidth(280);
  } else if (e.key === "End") {
    e.preventDefault();
    setListColWidth(720);
  }
}
</script>

<style scoped>
.record-list-wrapper {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
  position: relative;
}

.list-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  transition: padding-top var(--transition-smooth);
}

/* Cold-start-safe fade-in: a pure CSS animation (not a JS <Transition>), so the
   list is never held unmounted/invisible by a stalled transition while the window
   is hidden (WebView2 drops rAF). CSS animations resume & complete on their own. */
.list-body--enter {
  animation: list-body-enter var(--transition-smooth) ease;
}
@keyframes list-body-enter {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
:global(body.anim-disabled) .list-body--enter {
  animation: none;
}
@media (prefers-reduced-motion: reduce) {
  .list-body--enter {
    animation: none;
  }
}

/* Window-mode toolbar block: the batch bar floats just below it. */
.list-chrome {
  position: relative;
  flex-shrink: 0;
}
.list-chrome .batch-bar-holder {
  top: 100%;
}

.list-reload-bar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 6px 12px;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  background: color-mix(in srgb, var(--bg-elevated) 92%, transparent);
  border-bottom: 1px solid var(--border-subtle);
  pointer-events: none;
}

.record-list.reloading {
  opacity: 0.72;
  transition: opacity var(--transition-fast);
}

.preview-host {
  flex: 1.15;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.preview-host--drawer {
  position: absolute;
  inset: 0 0 0 auto;
  width: min(100%, 420px);
  max-width: 100%;
  z-index: 20;
  flex: none;
  box-shadow: var(--shadow-lg);
  border-left: 1px solid var(--border-subtle);
  animation: preview-drawer-in var(--transition-smooth);
}

:global(body.anim-disabled) .preview-host--drawer,
:global(body.anim-disabled) .preview-drawer-backdrop {
  animation: none;
}

@media (prefers-reduced-motion: reduce) {
  .preview-host--drawer,
  .preview-drawer-backdrop {
    animation: none;
  }
}

.preview-drawer-backdrop {
  position: absolute;
  inset: 0;
  z-index: 15;
  background: var(--overlay-bg);
  animation: fade-in var(--transition-fast);
}

@keyframes preview-drawer-in {
  from {
    transform: translateX(12px);
    opacity: 0.6;
  }
  to {
    transform: none;
    opacity: 1;
  }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.resizer {
  width: 4px;
  /* Overlay the list column's right edge instead of reserving flex space,
     keeping the list/preview layout fully compact. z-index keeps it above
     the column's positioned rows so hover/drag pointer events still land. */
  margin-left: -4px;
  position: relative;
  z-index: 10;
  cursor: col-resize;
  background: transparent;
  flex-shrink: 0;
  transition: background var(--transition-fast);
  touch-action: none;
}

.resizer:hover,
.resizer.active {
  background: var(--accent);
}

.list-column {
  flex: 1.35;
  min-width: 200px;
  max-width: 520px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  /* Same surface as preview — sidebar stays elevated for nav hierarchy. */
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
}

.list-column--full {
  flex: 1;
  max-width: none;
  width: auto;
  min-width: 0;
  border-right: none;
}

.resizer:focus-visible {
  background: var(--accent);
  outline: none;
}

.record-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-1) 0 var(--space-2);
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
  gap: 8px; /* must match GRID_GAP in script */
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
  font-size: var(--text-xs, 0.625rem);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--pin);
  padding: var(--space-3) var(--space-4) var(--space-1);
  display: flex;
  align-items: center;
  gap: var(--space-1);
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

/* —— Back to top —— */
.back-to-top-btn {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-4);
  z-index: 5;
  width: 32px;
  height: 32px;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: var(--shadow-md);
  transition:
    background var(--transition-fast),
    color var(--transition-fast),
    border-color var(--transition-fast);
}

.back-to-top-btn:hover {
  background: var(--bg-hover);
  color: var(--accent-text);
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
}

.back-to-top-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.back-top-enter-active,
.back-top-leave-active {
  transition: opacity var(--transition-normal), transform var(--transition-normal);
}

.back-top-enter-from,
.back-top-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
