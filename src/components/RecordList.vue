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
        <ListToolbar :list-layout="listLayout" @set-layout="setListLayout" />

        <Transition name="fade">
          <BatchBar v-if="clipboardStore.batchMode" />
        </Transition>
      </template>

      <!-- Loading / Empty -->
      <ListEmptyState v-if="isEmptyOrLoading" />

      <!-- Record List (windowed: only mount rows near the viewport) -->
      <div
        v-else
        class="list-body"
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
          :class="{ 'view-grid': listLayout === 'grid', reloading: isListReloading }"
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
        <div
          v-else
          :id="`record-option-${item.record!.id}`"
          class="record-item"
          role="option"
          :aria-selected="clipboardStore.batchMode ? clipboardStore.selectedIds.has(item.record!.id) : clipboardStore.selectedId === item.record!.id"
          :tabindex="isOptionTabbable(item.record!.id) ? 0 : -1"
          :class="{
            selected: clipboardStore.selectedId === item.record!.id && !clipboardStore.batchMode,
            'batch-mode': clipboardStore.batchMode,
            'batch-checked': clipboardStore.batchMode && clipboardStore.selectedIds.has(item.record!.id),
            'is-text': item.record!.content_type === 'text',
            'is-link': item.record!.content_type === 'link',
            'is-code': item.record!.content_type === 'code',
            'is-image': item.record!.content_type === 'image',
            'is-file': item.record!.content_type === 'file',
            'is-new': item.record!.id === clipboardStore.lastIncomingId,
            'is-leaving': leavingIds.has(item.record!.id),
          }"
          :data-record-id="item.record!.id"
          @click="onItemClick(item.record!.id)"
          @contextmenu.prevent="showContextMenu($event, item.record!)"
          @keydown.enter.prevent.stop="onItemActivate(item.record!.id)"
          @keydown.space.prevent="onItemClick(item.record!.id)"
        >
          <div
            v-if="clipboardStore.batchMode"
            class="record-checkbox"
            :class="{ checked: clipboardStore.selectedIds.has(item.record!.id) }"
            aria-hidden="true"
          >
            <span v-if="clipboardStore.selectedIds.has(item.record!.id)">✓</span>
          </div>

          <!-- Type color chip; standalone CSS color shows a swatch instead -->
          <div
            v-if="rowColor(item.record!)"
            class="record-color-swatch"
            :style="{ background: rowColor(item.record!)! }"
            :title="rowColor(item.record!)!"
            aria-hidden="true"
          />
          <div
            v-else
            class="record-type-icon type-chip"
            :class="item.record!.content_type"
            aria-hidden="true"
          >
            <TypeIcon :type="item.record!.content_type" :size="14" />
          </div>

          <div class="record-body">
            <div
              v-if="item.record!.content_type === 'image' && item.thumb"
              class="record-image-tile"
              aria-hidden="true"
            >
              <img
                class="record-thumb"
                :src="item.thumb"
                alt=""
                loading="lazy"
                decoding="async"
              />
            </div>
            <div
              v-else
              class="record-title"
              :title="recordTitleAttr(item.record!)"
              v-html="previewHtml(item.record!)"
            ></div>
            <div class="record-meta">
              <span class="record-time">{{ formatTime(item.record!.created_at) }}</span>
              <span class="record-source">
                <SourceBadge
                  :source-app="item.record!.source_app"
                  :label-html="sourceLabelHtml(item.record!)"
                />
              </span>
              <span
                v-if="item.record!.content_type === 'image' && item.record!.width && item.record!.height"
                class="record-dims"
              >{{ item.record!.width }}×{{ item.record!.height }}</span>
              <span v-if="item.record!.is_sensitive" class="record-sensitive">{{ $t('record.sensitive') }}</span>
            </div>
          </div>

          <div class="record-actions" @click.stop>
            <template v-if="clipboardStore.trashFilter">
              <button
                type="button"
                class="record-action-btn"
                :aria-label="$t('record.restoreRecord')"
                :title="$t('record.restoreRecord')"
                @click="onRowRestore(item.record!.id)"
              ><AppIcon name="restore" :size="13" /></button>
              <button
                type="button"
                class="record-action-btn danger"
                :aria-label="$t('record.permanentDelete')"
                :title="$t('record.permanentDelete')"
                @click="quickDelete(item.record!)"
              ><AppIcon name="trash" :size="13" /></button>
            </template>
            <template v-else>
              <button
                type="button"
                class="record-action-btn"
                :aria-label="$t('record.pasteLabel')"
                :title="$t('record.pasteLabel')"
                @click="quickPaste(item.record!.id)"
              ><AppIcon name="paste" :size="13" /></button>
              <button
                type="button"
                class="record-action-btn"
                :class="{ starred: item.record!.is_favorite }"
                :aria-label="item.record!.is_favorite ? $t('record.unfavorite') : $t('record.favorite')"
                :title="item.record!.is_favorite ? $t('record.unfavorite') : $t('record.favorite')"
                @click="onRowFavorite(item.record!.id)"
              ><AppIcon name="star" :size="13" :fill="item.record!.is_favorite ? 'currentColor' : 'none'" /></button>
              <button
                type="button"
                class="record-action-btn"
                :class="{ active: isPinned(item.record!) }"
                :aria-label="isPinned(item.record!) ? $t('record.unpin') : $t('record.pin')"
                :title="isPinned(item.record!) ? $t('record.unpin') : $t('record.pin')"
                @click="scheduleTogglePin(item.record!)"
              ><AppIcon name="pin" :size="13" :fill="isPinned(item.record!) ? 'currentColor' : 'none'" /></button>
              <button
                type="button"
                class="record-action-btn danger"
                :aria-label="$t('record.deleteRecord')"
                :title="$t('record.deleteRecord')"
                @click="quickDelete(item.record!)"
              ><AppIcon name="trash" :size="13" /></button>
            </template>
          </div>
        </div>
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
import { computed, reactive, ref, watch, nextTick, shallowRef, onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import PreviewPane from "./PreviewPane.vue";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu.vue";
import AliasDialog from "./AliasDialog.vue";
import BatchBar from "./BatchBar.vue";
import SourceBadge from "./SourceBadge.vue";
import AppIcon from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import ListToolbar from "./ListToolbar.vue";
import ListEmptyState from "./ListEmptyState.vue";
import { sourceShortName } from "../utils/sourceBadge";
import { parseClipboardColor } from "../utils/clipboardColor";
import type { ClipboardRecord } from "../types";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useVirtualList, type ListLayout } from "../composables/useVirtualList";
import { useColumnResize } from "../composables/useColumnResize";
import { useI18n } from "vue-i18n";
import {
  escapeHtml,
  highlightSearchHtml,
  highlightedPreview,
} from "../utils/highlightSearch";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();
const listRef = ref<HTMLElement | null>(null);

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

/** Optimistic pin icon before list reorders (spec §3.3). */
const pinOverride = shallowRef(new Map<number, boolean>());
/** Rows fading out before soft-delete (spec §3.4, restrained). */
const leavingIds = shallowRef(new Set<number>());

function isPinned(record: ClipboardRecord): boolean {
  return pinOverride.value.get(record.id) ?? record.is_pinned;
}

function sleep(ms: number): Promise<void> {
  if (!settingsStore.settings.enable_animation) return Promise.resolve();
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return Promise.resolve();
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function scheduleTogglePin(record: ClipboardRecord) {
  if (leavingIds.value.has(record.id)) return;
  const next = !isPinned(record);
  const pending = new Map(pinOverride.value);
  pending.set(record.id, next);
  pinOverride.value = pending;
  await sleep(150);
  const result = await clipboardStore.togglePin(record.id);
  const cleared = new Map(pinOverride.value);
  cleared.delete(record.id);
  pinOverride.value = cleared;
  if (result == null) toast(t('common.operationFailed'), "error");
}

const LAYOUT_KEY = "clipvault-list-layout";

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
  listLayout.value = mode;
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

/** Back-to-top: reveal once scrolled past a fixed threshold (~1 viewport). */
const BACK_TO_TOP_THRESHOLD = 400;

const showBackToTop = computed(
  () => !isEmptyOrLoading.value && scrollTop.value > BACK_TO_TOP_THRESHOLD
);

function scrollToTop() {
  const el = listRef.value;
  if (!el) return;
  const reduceMotion =
    !settingsStore.settings.enable_animation ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollTo({ top: 0, behavior: reduceMotion ? "auto" : "smooth" });
}

const activeDescendantId = computed(() =>
  clipboardStore.selectedId != null ? `record-option-${clipboardStore.selectedId}` : undefined
);

const firstRecordId = computed(() => {
  for (const it of flatItems.value) {
    if (it.type === "record" && it.id != null) return it.id;
  }
  return null;
});

function isOptionTabbable(id: number): boolean {
  if (clipboardStore.selectedId === id) return true;
  if (clipboardStore.selectedId == null && firstRecordId.value === id) return true;
  return false;
}

function sourceLabelHtml(record: ClipboardRecord): string | undefined {
  const q = clipboardStore.searchQuery.trim();
  if (!q) return undefined;
  return highlightSearchHtml(sourceShortName(record.source_app), q);
}

/** Text that is only a CSS color → list swatch instead of type icon. */
function rowColor(record: ClipboardRecord): string | null {
  if (record.content_type !== "text") return null;
  return parseClipboardColor(record.content);
}

watch(
  () => clipboardStore.selectedId,
  async (id) => {
    if (id == null) return;
    await nextTick();
    const list = listRef.value;
    if (!list) return;
    const mounted = list.querySelector(`[data-record-id="${id}"]`) as HTMLElement | null;
    if (mounted) {
      mounted.scrollIntoView({ block: "nearest" });
      return;
    }
    // Selected row may be outside the virtual window — jump by layout offset.
    const target = flatItems.value.find((it) => it.id === id);
    if (!target) return;
    const viewH = list.clientHeight;
    const top = target.offset;
    const bottom = top + target.height;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + viewH) list.scrollTop = bottom - viewH;
    scrollTop.value = list.scrollTop;
  }
);

const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  record: null as ClipboardRecord | null,
});

const aliasDialog = reactive({
  visible: false,
  recordId: null as number | null,
  initialAlias: "",
});

function openAliasDialog(record: ClipboardRecord) {
  aliasDialog.recordId = record.id;
  aliasDialog.initialAlias = record.alias ?? "";
  aliasDialog.visible = true;
}

function closeAliasDialog() {
  aliasDialog.visible = false;
  aliasDialog.recordId = null;
  aliasDialog.initialAlias = "";
}

const contextMenuItems = computed<ContextMenuItem[]>(() => {
  if (clipboardStore.trashFilter) {
    return [
      { id: "restore", label: t('common.restore'), icon: "restore" },
      { id: "permanentDelete", label: t('record.permanentDelete'), icon: "trash", danger: true, separatorBefore: true },
    ];
  }
  const rec = contextMenu.record;
  return [
    { id: "paste", label: t('common.paste'), icon: "paste", shortcut: "Enter" },
    { id: "pastePlain", label: t('common.pastePlain'), icon: "type", shortcut: "Alt+V" },
    {
      id: "favorite",
      label: rec?.is_favorite ? t('record.unfavorite') : t('record.favorite'),
      icon: "star",
      shortcut: "Ctrl+D",
      separatorBefore: true,
    },
    {
      id: "pin",
      label: rec?.is_pinned ? t('record.unpin') : t('record.pin'),
      icon: "pin",
      shortcut: "Ctrl+T",
    },
    {
      id: "alias",
      label: rec?.alias?.trim() ? t('record.editAlias') : t('record.setAlias'),
      icon: "edit",
    },
    { id: "delete", label: t('common.delete'), icon: "trash", shortcut: "Del / ⌫", danger: true, separatorBefore: true },
  ];
});

function recordAlias(record: ClipboardRecord): string {
  return (record.alias ?? "").trim();
}

function contentPreview(record: ClipboardRecord): string {
  if (record.content_type === "image") {
    if (record.width && record.height) {
      return t('record.imageLabel', { w: record.width, h: record.height });
    }
    return t('record.imageOnly');
  }
  const maxLen = 80;
  if (record.content.length <= maxLen) return record.content;
  return record.content.slice(0, maxLen) + "…";
}

/** List primary line: alias when set, otherwise content preview. */
function getPreview(record: ClipboardRecord): string {
  const alias = recordAlias(record);
  if (alias) return alias.length > 80 ? alias.slice(0, 80) + "…" : alias;
  return contentPreview(record);
}

/** Hover shows original content when an alias is displayed. */
function recordTitleAttr(record: ClipboardRecord): string | undefined {
  if (!recordAlias(record)) return undefined;
  return contentPreview(record);
}

/** Safe HTML for list title — highlights search hits when querying. */
function previewHtml(record: ClipboardRecord): string {
  const alias = recordAlias(record);
  const q = clipboardStore.searchQuery.trim();
  if (alias) {
    if (!q) return escapeHtml(getPreview(record));
    return highlightedPreview(alias, q, 80);
  }
  if (record.content_type === "image") {
    return escapeHtml(getPreview(record));
  }
  if (!q) return escapeHtml(getPreview(record));
  return highlightedPreview(record.content, q, 80);
}

async function quickPaste(id: number) {
  try {
    await clipboardStore.pasteRecord(id);
    toast(t('record.pasted'), "success");
  } catch {
    toast(t('record.pasteFailed'), "error");
  }
}

async function onRowFavorite(id: number) {
  const next = await clipboardStore.toggleFavorite(id);
  if (next == null) toast(t('common.operationFailed'), "error");
}

async function onRowRestore(id: number) {
  try {
    await clipboardStore.restoreRecord(id);
  } catch {
    toast(t('common.operationFailed'), "error");
  }
}

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

async function quickDelete(record: ClipboardRecord) {
  if (clipboardStore.trashFilter) {
    const ok = await confirm({
      title: t('record.permanentDelete'),
      message: t('record.permanentDeleteMsg'),
      confirmText: t('record.permanentDelete'),
      danger: true,
    });
    if (ok) {
      try {
        await clipboardStore.permanentlyDeleteRecord(record.id);
        toast(t('record.deletedPermanently'), "success");
      } catch {
        toast(t('common.operationFailed'), "error");
      }
    }
    return;
  }
  if (leavingIds.value.has(record.id)) return;
  const nextLeave = new Set(leavingIds.value);
  nextLeave.add(record.id);
  leavingIds.value = nextLeave;
  await sleep(160);
  try {
    await clipboardStore.deleteRecord(record.id);
    toast(t('record.deleted'), "success");
  } catch {
    toast(t('common.operationFailed'), "error");
  } finally {
    const cleared = new Set(leavingIds.value);
    cleared.delete(record.id);
    leavingIds.value = cleared;
  }
}

let cachedNow = Date.now();
let cachedNowTimer: ReturnType<typeof setTimeout> | null = null;

function getNow(): number {
  // Refresh the cached "now" at most once per 30s to avoid creating a Date
  // object per row on every render.
  if (!cachedNowTimer) {
    cachedNowTimer = setTimeout(() => {
      cachedNow = Date.now();
      cachedNowTimer = null;
    }, 30_000);
  }
  return cachedNow;
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  const diffMs = getNow() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return t('record.justNow');
  if (diffMin < 60) return t('record.minutesAgo', { n: diffMin });
  if (diffMin < 1440) return t('record.hoursAgo', { n: Math.floor(diffMin / 60) });
  return d.toLocaleDateString(undefined, { month: "numeric", day: "numeric" });
}

function onItemClick(id: number) {
  if (clipboardStore.batchMode) {
    clipboardStore.toggleBatchSelect(id);
    return;
  }
  clipboardStore.selectRecord(id);
}

/** Enter activates paste (or restore in trash). Double-click removed — easy to misfire. */
async function onItemActivate(id: number) {
  if (clipboardStore.batchMode) {
    clipboardStore.toggleBatchSelect(id);
    return;
  }
  if (clipboardStore.trashFilter) {
    try {
      await clipboardStore.restoreRecord(id);
    } catch {
      toast(t('common.operationFailed'), "error");
    }
    return;
  }
  try {
    await clipboardStore.pasteRecord(id);
    toast(t('record.pasted'), "success");
  } catch {
    toast(t('record.pasteFailed'), "error");
  }
}

function showContextMenu(e: MouseEvent, record: ClipboardRecord) {
  contextMenu.visible = true;
  contextMenu.x = e.clientX;
  contextMenu.y = e.clientY;
  contextMenu.record = record;
}

async function onContextSelect(id: string) {
  const record = contextMenu.record;
  contextMenu.visible = false;
  if (!record) return;

  if (id === "paste") {
    try {
      await clipboardStore.pasteRecord(record.id);
      toast(t('record.pasted'), "success");
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
    return;
  }
  if (id === "pastePlain") {
    try {
      await clipboardStore.pasteRecord(record.id, "plain");
      toast(t('record.pastedPlain'), "success");
    } catch {
      toast(t('record.pasteFailed'), "error");
    }
    return;
  }
  if (id === "favorite") {
    const next = await clipboardStore.toggleFavorite(record.id);
    if (next == null) toast(t('common.operationFailed'), "error");
    return;
  }
  if (id === "pin") {
    await scheduleTogglePin(record);
    return;
  }
  if (id === "alias") {
    openAliasDialog(record);
    return;
  }
  if (id === "restore") {
    await clipboardStore.restoreRecord(record.id);
    return;
  }
  if (id === "delete") {
    await quickDelete(record);
    return;
  }
  if (id === "permanentDelete") {
    const ok = await confirm({
      title: t('record.permanentDelete'),
      message: t('record.permanentDeleteMsg'),
      confirmText: t('record.permanentDelete'),
      danger: true,
    });
    if (ok) {
      try {
        await clipboardStore.permanentlyDeleteRecord(record.id);
        toast(t('record.deletedPermanently'), "success");
      } catch {
        toast(t('common.operationFailed'), "error");
      }
    }
  }
}

function closeContextMenu() {
  contextMenu.visible = false;
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
  cursor: col-resize;
  background: transparent;
  flex-shrink: 0;
  transition: background 0.15s;
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

.view-grid .record-item {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  margin: 0;
  /* padding/gap/height coupled to GRID_CARD_HEIGHT in script — keep in sync */
  padding: 10px;
  gap: 6px;
  /* Scale with --ui-font-scale (matches useVirtualList gridCardHeight) */
  height: calc(132px * var(--ui-font-scale, 1));
  max-height: calc(132px * var(--ui-font-scale, 1));
  box-sizing: border-box;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
}

.view-grid .record-item:hover {
  background: var(--bg-hover);
  border-color: var(--border-default);
  box-shadow: none;
}

.view-grid .record-item.selected {
  background: color-mix(in srgb, var(--accent) 12%, var(--bg-surface));
  border-color: color-mix(in srgb, var(--accent) 32%, transparent);
  box-shadow: none;
}

.view-grid .record-item.is-image {
  height: calc(140px * var(--ui-font-scale, 1));
  max-height: calc(140px * var(--ui-font-scale, 1));
}

.view-grid .record-item.batch-mode {
  padding: 10px;
}

.view-grid .record-checkbox {
  left: auto;
  right: var(--space-2);
  top: var(--space-2);
  z-index: 3;
  width: 18px;
  height: 18px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  border-color: var(--border-default);
  box-shadow: var(--shadow-sm);
}

.view-grid .record-checkbox.checked {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.view-grid .record-item.batch-mode .record-type-icon {
  margin-left: 0;
}

.view-grid .record-item.batch-checked {
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  background: color-mix(in srgb, var(--accent) 10%, var(--bg-surface));
  box-shadow: none;
}

.view-grid .record-item.batch-mode .record-actions {
  display: none;
}

/* Image cards: thumb on top (original); hide side type chip */
.view-grid .record-item.is-image .record-type-icon {
  display: none;
}

.view-grid .record-type-icon {
  width: 28px;
  height: 28px;
  margin-top: 0;
  flex-shrink: 0;
}

.view-grid .record-color-swatch {
  width: 28px;
  height: 28px;
  margin-top: 0;
}

.view-grid .record-body {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  width: 100%;
  min-width: 0;
  min-height: 0;
  gap: var(--space-1);
  overflow: hidden;
}

.view-grid .record-image-tile {
  order: -1;
  width: 100%;
  height: 72px;
  max-height: 72px;
  flex: 0 0 72px;
  overflow: hidden;
}

.view-grid .record-title {
  flex: 1 1 auto;
  min-height: 0;
  max-height: calc(1.35em * 2);
  white-space: normal;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.35;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.view-grid .record-meta {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  margin-top: auto;
  gap: 6px;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  flex-shrink: 0;
}

.view-grid .record-time {
  flex-shrink: 0;
}

.view-grid .record-source {
  flex: 1 1 auto;
  min-width: 0;
  max-width: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.view-grid .record-dims {
  display: none; /* keep meta to a single tight line in grid */
}

.view-grid .record-sensitive {
  flex-shrink: 0;
  max-width: 3.5rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.view-grid .record-actions {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  margin: 0;
  z-index: 2;
  max-width: calc(100% - 12px);
  overflow: hidden;
  background: color-mix(in srgb, var(--bg-surface) 94%, transparent);
  border-radius: var(--radius-sm);
  padding: 1px;
  box-shadow: var(--shadow-sm);
}

.view-grid .record-action-btn {
  width: 26px;
  height: 26px;
  flex-shrink: 0;
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

.record-item {
  --row-accent: var(--accent);
  /* padding/margin coupled to BASE_ROW_HEIGHT in script — keep in sync */
  padding: 10px 12px;
  margin: 0 4px 2px;
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition:
    background var(--transition-fast),
    opacity var(--transition-fast),
    transform var(--transition-fast);
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  position: relative;
  border: 1px solid transparent;
  background: transparent;
  box-shadow: none;
}

.record-item.is-text { --row-accent: var(--type-text); }
.record-item.is-code { --row-accent: var(--type-code); }
.record-item.is-link { --row-accent: var(--type-link); }
.record-item.is-image { --row-accent: var(--type-image); }
.record-item.is-file { --row-accent: var(--type-file); }

.record-item:hover {
  background: var(--bg-hover);
}

.record-item.selected {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}

.record-item.is-leaving {
  opacity: 0;
  transform: translateX(-4px);
  pointer-events: none;
}

/* Freshly captured row: brief accent flash as capture confirmation. */
.record-item.is-new {
  animation: row-flash 900ms ease-out;
}

@keyframes row-flash {
  from {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  to {
    background: transparent;
  }
}

.record-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.record-item.batch-mode {
  padding-left: 32px;
}

.record-checkbox {
  position: absolute;
  left: 10px;
  top: 16px;
  width: 14px;
  height: 14px;
  border: 1.5px solid var(--text-tertiary);
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-xs);
  color: transparent;
  transition: all var(--transition-fast);
  flex-shrink: 0;
}

.record-checkbox.checked {
  background: var(--accent);
  border-color: var(--accent);
  color: white;
}

/* Type color chip */
.record-type-icon {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm, 6px);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-top: 1px;
}

.record-color-swatch {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm, 6px);
  flex-shrink: 0;
  margin-top: 1px;
  border: 1px solid var(--border-default);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 10%, transparent);
}

/* Type icon coloring is provided by the shared .type-chip utility in
   main.css (single source of truth for content-type colors). */

/* Image thumb in body (design: type icon left, preview right) */
.record-image-tile {
  width: 64px;
  height: 48px;
  border-radius: var(--radius-sm, 6px);
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
}

.record-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.record-body {
  flex: 1;
  min-width: 0;
}

.record-title {
  font-size: var(--text-base, 0.8125rem);
  font-weight: 500;
  color: var(--text-primary);
  line-height: 1.4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.record-item.is-link .record-title {
  color: var(--type-link);
  text-decoration: underline;
  text-decoration-color: color-mix(in srgb, var(--type-link) 35%, transparent);
  text-underline-offset: 2px;
}

.record-item.is-code .record-title {
  font-family: var(--font-mono);
  font-weight: 400;
  font-size: var(--text-md, 0.75rem);
}

.record-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: 6px;
  font-size: var(--text-sm, 0.6875rem);
  color: var(--text-tertiary);
}

.record-time {
  white-space: nowrap;
}

.record-source {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 160px;
}

.record-dims {
  white-space: nowrap;
  opacity: 0.85;
}

.record-sensitive {
  font-size: var(--text-xs, 0.625rem);
  font-weight: 600;
  color: var(--sensitive);
  background: var(--sensitive-soft);
  padding: 1px 6px;
  border-radius: 4px;
}

/* Hover quick actions — paste / star / pin / trash */
.record-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--transition-fast);
  margin-top: -2px;
}

.record-item:hover .record-actions,
.record-item:focus-within .record-actions,
.record-item.selected .record-actions,
.record-actions:has(.active),
.record-actions:has(.starred) {
  opacity: 1;
  pointer-events: auto;
}

/* When collapsed to status-only, hide inert buttons */
.record-item:not(:hover):not(:focus-within):not(.selected) .record-action-btn:not(.active):not(.starred) {
  display: none;
}

.record-action-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.record-action-btn:hover {
  background: var(--bg-hover);
  color: var(--accent-text);
}

.record-action-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 0;
}

.record-action-btn.active {
  color: var(--pin);
}

.record-action-btn.starred {
  color: var(--warning);
}

.record-action-btn.danger:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

/* Always show active pin/star even when row not hovered */
.record-action-btn.active,
.record-action-btn.starred {
  opacity: 1;
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
