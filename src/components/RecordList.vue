<template>
  <div ref="wrapperRef" class="record-list-wrapper">
    <div
      class="list-column"
      :class="{ 'list-column--full': listIsFull }"
      :style="listColStyle"
    >
      <!-- Middle-column chrome (window mode): matches design list toolbar -->
      <div class="list-chrome">
        <ListToolbar :list-layout="listLayout" @set-layout="setListLayout" />

        <Transition name="batch-bar">
          <div v-if="clipboardStore.batchMode" ref="batchBarRef" class="batch-bar-holder">
            <BatchBar />
          </div>
        </Transition>
      </div>

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
        <RecordVirtualList
          :layout="listLayout"
          :grid-cols="gridCols"
          :display-items="displayItems"
          :pad-top="virtualPadTop"
          :pad-bottom="virtualPadBottom"
          :reloading="isListReloading"
          :fade-armed="layoutFadeArmed"
          :fade-on="layoutFadeOn"
          :scroll-el="setScrollEl"
          :leaving-ids="leavingIds"
          :source-overrides="sourceOverrides"
          :active-descendant-id="activeDescendantId"
          :is-pinned="isPinned"
          :is-option-tabbable="isOptionTabbable"
          :measure-row="measureRow"
          :set-pinned-block-el="setPinnedBlockEl"
          @scroll="onListScroll"
          @item-click="onItemClick"
          @item-activate="onItemActivate"
          @item-context-menu="showContextMenu"
          @item-paste="quickPaste"
          @item-favorite="onRowFavorite"
          @item-toggle-pin="scheduleTogglePin"
          @item-delete="quickDelete"
          @item-restore="onRowRestore"
          @docked="pinnedDocked = $event"
        />
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

    <!-- Preview area: resizer + side-by-side column / drawer overlay -->
    <PreviewHost
      :visible="previewChrome.kind !== 'hidden'"
      :drawer="previewChrome.kind === 'drawer'"
      :show-host="previewChrome.kind !== 'hidden'"
      :show-resizer="previewChrome.kind === 'column'"
      :fixed-column="previewChrome.kind === 'column' && previewChrome.sizing === 'fixed'"
      :col-width="splitColWidth"
      :col-min="splitColMin"
      :col-max="splitColMax"
      :dragging="splitDragging"
      @close="clipboardStore.clearSelection()"
      @resize-start="onSplitResizeStart"
      @resize-key="onSplitResizeKey"
    />

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
import PreviewHost from "./PreviewHost.vue";
import ContextMenu from "./ContextMenu.vue";
import AliasDialog from "./AliasDialog.vue";
import BatchBar from "./BatchBar.vue";
import AppIcon from "./icons/AppIcon.vue";
import RecordVirtualList from "./RecordVirtualList.vue";
import ListToolbar from "./ListToolbar.vue";
import ListEmptyState from "./ListEmptyState.vue";
import { useVirtualList, type ListLayout } from "../composables/useVirtualList";
import { useColumnResize } from "../composables/useColumnResize";
import { useBatchBarHeight } from "../composables/useBatchBarHeight";
import { useRecordActions } from "../composables/useRecordActions";
import { buildSourceOverrides } from "../utils/sourceBadge";
import {
  LIST_COL_DEFAULT,
  LIST_COL_MAX,
  LIST_COL_MIN,
  LIST_MIN,
  PREVIEW_DEFAULT,
  PREVIEW_MAX,
  PREVIEW_MIN,
  clampPreviewWidth,
  normalizePreviewLayoutPref,
  resolvePreviewChrome,
} from "../utils/previewLayout";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const listRef = ref<HTMLElement | null>(null);

/** Callback ref so RecordVirtualList can forward its scroll element up —
 * useVirtualList / useRecordActions need direct element access. */
function setScrollEl(el: unknown) {
  listRef.value = el as HTMLElement | null;
}

const sourceOverrides = computed(() =>
  buildSourceOverrides(settingsStore.settings.source_name_overrides),
);

// --- Floating batch bar (window mode): reserve its height as list padding ---
const batchBarRef = ref<HTMLElement | null>(null);
const { height: batchBarHeight } = useBatchBarHeight(batchBarRef);

const wrapperRef = ref<HTMLElement | null>(null);
const wrapperWidth = ref(0);
let wrapperRo: ResizeObserver | null = null;
const previewPref = computed(() =>
  normalizePreviewLayoutPref(settingsStore.settings.preview_layout),
);
const previewChrome = computed(() =>
  resolvePreviewChrome(
    previewPref.value,
    !!clipboardStore.selectedRecord,
    clipboardStore.batchMode,
    wrapperWidth.value,
  ),
);
const listIsFull = computed(() => previewChrome.value.kind !== "column");
const listColStyle = computed(() => {
  if (previewChrome.value.kind !== "column" || previewChrome.value.sizing !== "flex") {
    return undefined;
  }
  return {
    width: `${listColWidth.value}px`,
    minWidth: `${listColWidth.value}px`,
    maxWidth: "none",
    flex: "none",
  };
});

const {
  width: listColWidth,
  isDragging: listColDragging,
  startResize: startListColResize,
  setWidth: setListColWidth,
} = useColumnResize({
  storageKey: "clipboard-list-col-width",
  defaultWidth: LIST_COL_DEFAULT,
  min: LIST_COL_MIN,
  max: LIST_COL_MAX,
});
const {
  width: previewWidth,
  isDragging: previewDragging,
  startResize: startPreviewResize,
  setWidth: setPreviewWidth,
} = useColumnResize({
  storageKey: "clipboard-preview-col-width",
  defaultWidth: PREVIEW_DEFAULT,
  min: PREVIEW_MIN,
  max: PREVIEW_MAX,
  invert: true,
});
const previewMaxFit = computed(() =>
  wrapperWidth.value > 0
    ? Math.max(PREVIEW_MIN, Math.min(PREVIEW_MAX, wrapperWidth.value - LIST_MIN))
    : PREVIEW_MAX,
);
const previewColWidth = computed(() =>
  clampPreviewWidth(previewWidth.value, wrapperWidth.value),
);
const splitUsesListWidth = computed(
  () => previewChrome.value.kind === "column" && previewChrome.value.sizing === "flex",
);
const splitColWidth = computed(() =>
  splitUsesListWidth.value ? listColWidth.value : previewColWidth.value,
);
const splitColMin = computed(() =>
  splitUsesListWidth.value ? LIST_COL_MIN : PREVIEW_MIN,
);
const splitColMax = computed(() =>
  splitUsesListWidth.value ? LIST_COL_MAX : previewMaxFit.value,
);
const splitDragging = computed(() =>
  splitUsesListWidth.value ? listColDragging.value : previewDragging.value,
);

onMounted(() => {
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
const pinnedDocked = ref(false);

const {
  displayItems,
  virtualPadTop,
  virtualPadBottom,
  flatItems,
  gridCols,
  scrollTop,
  onListScroll,
  fillViewportIfNeeded,
  measureRow,
  setPinnedBlockEl,
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
  pinnedDocked: () => pinnedDocked.value,
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
    setListColWidth(LIST_COL_MIN);
  } else if (e.key === "End") {
    e.preventDefault();
    setListColWidth(LIST_COL_MAX);
  }
}

function onPreviewResizeKey(e: KeyboardEvent) {
  const step = e.shiftKey ? 40 : 16;
  if (e.key === "ArrowLeft") {
    e.preventDefault();
    setPreviewWidth(previewColWidth.value + step);
  } else if (e.key === "ArrowRight") {
    e.preventDefault();
    setPreviewWidth(previewColWidth.value - step);
  } else if (e.key === "Home") {
    e.preventDefault();
    setPreviewWidth(PREVIEW_MIN);
  } else if (e.key === "End") {
    e.preventDefault();
    setPreviewWidth(previewMaxFit.value);
  }
}

function onSplitResizeStart(e: PointerEvent) {
  if (splitUsesListWidth.value) startListColResize(e);
  else startPreviewResize(e);
}

function onSplitResizeKey(e: KeyboardEvent) {
  if (splitUsesListWidth.value) onListColResizeKey(e);
  else onPreviewResizeKey(e);
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

/* Record-list and preview-area styles live in RecordVirtualList.vue /
   PreviewHost.vue alongside their markup. */

.list-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  /* Same surface as preview — sidebar stays elevated for nav hierarchy. */
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
}

.list-column:not(.list-column--full) {
  min-width: 280px;
}

.list-column--full {
  border-right: none;
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
