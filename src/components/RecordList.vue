<template>
  <div class="record-list-wrapper">
    <div class="list-column">
      <!-- Middle-column chrome (window mode): matches design list toolbar -->
      <template v-if="showListChrome">
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
            >清空回收站</button>
            <select
              class="list-sort"
              :value="clipboardStore.listSort"
              title="列表排序"
              aria-label="列表排序"
              @change="onSortChange"
            >
              <option
                v-for="opt in LIST_SORT_OPTIONS"
                :key="opt.value"
                :value="opt.value"
              >{{ opt.label }}</option>
            </select>
            <div class="view-toggle" role="group" aria-label="视图切换">
              <button
                type="button"
                class="view-toggle-btn"
                :class="{ active: listLayout === 'list' }"
                title="列表视图"
                aria-label="列表视图"
                :aria-pressed="listLayout === 'list'"
                @click="setListLayout('list')"
              ><AppIcon name="list" :size="14" /></button>
              <button
                type="button"
                class="view-toggle-btn"
                :class="{ active: listLayout === 'grid' }"
                title="网格视图"
                aria-label="网格视图"
                :aria-pressed="listLayout === 'grid'"
                @click="setListLayout('grid')"
              ><AppIcon name="grid" :size="14" /></button>
            </div>
            <button
              type="button"
              class="list-tool-btn"
              :class="{ active: clipboardStore.batchMode }"
              title="批量操作"
              aria-label="批量操作"
              :aria-pressed="clipboardStore.batchMode"
              @click="toggleBatchMode"
            ><AppIcon name="batch" :size="14" /></button>
          </div>
        </div>

        <Transition name="fade">
          <BatchBar v-if="clipboardStore.batchMode" />
        </Transition>
      </template>

      <!-- Loading (initial only) -->
      <div v-if="clipboardStore.isLoading && clipboardStore.records.length === 0" class="loading-state">
        <div class="loading-spinner"></div>
        <span>加载中…</span>
      </div>

      <!-- Empty -->
      <div v-else-if="clipboardStore.filteredRecords.length === 0 && !clipboardStore.isLoading" class="empty-state">
        <div class="empty-icon"><AppIcon :name="emptyState.icon" :size="36" :stroke-width="1.5" /></div>
        <div class="empty-text">{{ emptyState.title }}</div>
        <div v-if="emptyState.hint" class="empty-hint">
          <template v-if="emptyState.clearSearch">
            试试其他关键词，或
            <button class="clear-link" @click="clipboardStore.search('')">清除搜索</button>
          </template>
          <template v-else>{{ emptyState.hint }}</template>
        </div>
      </div>

      <!-- Record List (windowed: only mount rows near the viewport) -->
      <div
        v-else
        class="record-list"
        :class="{ 'view-grid': listLayout === 'grid' }"
        ref="listRef"
        role="listbox"
        aria-label="剪贴板记录"
        :aria-activedescendant="activeDescendantId"
        tabindex="-1"
        @scroll="onListScroll"
      >
      <div
        v-if="listLayout === 'list'"
        class="virtual-spacer"
        :style="{ height: `${virtualPadTop}px` }"
        aria-hidden="true"
      />
      <template v-for="item in displayItems" :key="item.key">
        <div v-if="item.type === 'label'" class="section-label" aria-hidden="true"><AppIcon name="pin" :size="11" /> 置顶</div>
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
          :aria-selected="clipboardStore.selectedId === item.record!.id"
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
          @keydown.enter.prevent="onItemActivate(item.record!.id)"
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
            class="record-type-icon"
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
              <span v-if="item.record!.is_sensitive" class="record-sensitive">敏感</span>
            </div>
          </div>

          <div class="record-actions" @click.stop>
            <button
              v-if="!clipboardStore.trashFilter"
              type="button"
              class="record-action-btn"
              aria-label="粘贴"
              title="粘贴"
              @click="quickPaste(item.record!.id)"
            ><AppIcon name="paste" :size="13" /></button>
            <button
              type="button"
              class="record-action-btn"
              :class="{ starred: item.record!.is_favorite }"
              :aria-label="item.record!.is_favorite ? '取消收藏' : '收藏'"
              :title="item.record!.is_favorite ? '取消收藏' : '收藏'"
              @click="clipboardStore.toggleFavorite(item.record!.id)"
            ><AppIcon name="star" :size="13" :fill="item.record!.is_favorite ? 'currentColor' : 'none'" /></button>
            <button
              type="button"
              class="record-action-btn"
              :class="{ active: isPinned(item.record!) }"
              :aria-label="isPinned(item.record!) ? '取消置顶' : '置顶'"
              :title="isPinned(item.record!) ? '取消置顶' : '置顶'"
              @click="scheduleTogglePin(item.record!)"
            ><AppIcon name="pin" :size="13" :fill="isPinned(item.record!) ? 'currentColor' : 'none'" /></button>
            <button
              type="button"
              class="record-action-btn danger"
              :aria-label="clipboardStore.trashFilter ? '永久删除' : '删除'"
              :title="clipboardStore.trashFilter ? '永久删除' : '删除'"
              @click="quickDelete(item.record!)"
            ><AppIcon name="trash" :size="13" /></button>
          </div>
        </div>
      </template>
      <div
        v-if="listLayout === 'list'"
        class="virtual-spacer"
        :style="{ height: `${virtualPadBottom}px` }"
        aria-hidden="true"
      />

      <!-- Footer: load-more status only -->
      <div v-if="clipboardStore.isLoadingMore || clipboardStore.hasMore" class="list-footer">
        <span v-if="clipboardStore.isLoadingMore" class="footer-loading">
          <span class="loading-spinner small" aria-hidden="true"></span>加载更多…
        </span>
        <span v-else>继续滚动加载更多</span>
      </div>
      </div>
    </div>

    <!-- Preview Pane (right side) -->
    <PreviewPane v-if="clipboardStore.selectedRecord && !clipboardStore.batchMode" />

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
import { computed, reactive, ref, watch, nextTick, onMounted, onUnmounted, shallowRef } from "vue";
import { useClipboardStore, LIST_SORT_OPTIONS, type ListSort } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import PreviewPane from "./PreviewPane.vue";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu.vue";
import AliasDialog from "./AliasDialog.vue";
import BatchBar from "./BatchBar.vue";
import SourceBadge from "./SourceBadge.vue";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import { sourceShortName } from "../utils/sourceBadge";
import { parseClipboardColor } from "../utils/clipboardColor";
import type { ClipboardRecord } from "../types";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { useBatchActions } from "../composables/useBatchActions";
import { recordThumbSrc } from "../utils/mediaUrl";
import {
  escapeHtml,
  highlightSearchHtml,
  highlightedPreview,
} from "../utils/highlightSearch";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode } = useBatchActions();
const listRef = ref<HTMLElement | null>(null);

/** Optimistic pin icon before list reorders (spec §3.3). */
const pinOverride = shallowRef(new Map<number, boolean>());
/** Rows fading out before soft-delete (spec §3.4, restrained). */
const leavingIds = shallowRef(new Set<number>());

function isPinned(record: ClipboardRecord): boolean {
  return pinOverride.value.get(record.id) ?? record.is_pinned;
}

function sleep(ms: number): Promise<void> {
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
  if (result == null) toast("操作失败", "error");
}

type ListLayout = "list" | "grid";
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

const CATEGORY_TITLES: Record<string, string> = {
  all: "全部",
  text: "文本",
  image: "图片",
  file: "文件",
  link: "链接",
  code: "代码",
  favorites: "收藏",
  trash: "回收站",
};

const categoryTitle = computed(() => {
  if (clipboardStore.trashFilter) return "回收站";
  const typeKey = clipboardStore.activeFilter;
  const typePart =
    typeKey !== "all" ? CATEGORY_TITLES[typeKey] ?? typeKey : null;
  const tagPart = clipboardStore.activeTag;
  if (typePart && tagPart) return `${typePart} · ${tagPart}`;
  if (tagPart) return tagPart;
  if (typePart) return typePart;
  return "全部剪贴板";
});

const listCountLabel = computed(() => {
  if (clipboardStore.searchQuery) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? `已找到 ${n}+ 条` : `共 ${n} 条`;
  }
  if (clipboardStore.trashFilter) {
    return `共 ${clipboardStore.trashCount} 条`;
  }
  if (clipboardStore.activeTag) {
    const n = clipboardStore.filteredRecords.length;
    return clipboardStore.hasMore ? `已加载 ${n}+ 条` : `共 ${n} 条`;
  }
  if (clipboardStore.activeFilter === "favorites") {
    return `共 ${clipboardStore.filterCounts.favorites} 条`;
  }
  if (clipboardStore.activeFilter !== "all") {
    return `共 ${clipboardStore.filterCounts[clipboardStore.activeFilter]} 条`;
  }
  return `共 ${clipboardStore.filterCounts.all} 条`;
});

function onSortChange(e: Event) {
  const value = (e.target as HTMLSelectElement).value as ListSort;
  clipboardStore.setListSort(value);
}

async function onEmptyTrash() {
  const ok = await confirm({
    title: "清空回收站",
    message: "确定要清空回收站吗？所有已删除的记录将被永久删除，此操作不可恢复。",
    confirmText: "清空",
    danger: true,
  });
  if (ok) {
    try {
      await clipboardStore.emptyTrash();
      toast("回收站已清空", "success");
    } catch {
      toast("清空失败", "error");
    }
  }
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

/** Row estimates scaled with UI font size (settings.font_size → --ui-font-scale). */
const BASE_ROW_HEIGHT = 68;
const BASE_LABEL_HEIGHT = 28;
const BASE_DIVIDER_HEIGHT = 17;
const OVERSCAN = 6;
const rowHeight = computed(() =>
  Math.round(BASE_ROW_HEIGHT * (settingsStore.settings.font_size / 16))
);
const labelHeight = computed(() =>
  Math.round(BASE_LABEL_HEIGHT * (settingsStore.settings.font_size / 16))
);
const dividerHeight = computed(() =>
  Math.round(BASE_DIVIDER_HEIGHT * (settingsStore.settings.font_size / 16))
);

const scrollTop = ref(0);
const viewportHeight = ref(480);
let scrollRaf = 0;

function onListScroll() {
  const el = listRef.value;
  if (!el) return;
  if (scrollRaf) cancelAnimationFrame(scrollRaf);
  scrollRaf = requestAnimationFrame(() => {
    scrollRaf = 0;
    scrollTop.value = el.scrollTop;
    viewportHeight.value = el.clientHeight;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 100) {
      void clipboardStore.loadMore();
    }
  });
}

/** If list shorter than viewport, keep fetching until filled or exhausted. */
async function fillViewportIfNeeded() {
  await nextTick();
  const el = listRef.value;
  if (!el || !clipboardStore.hasMore || clipboardStore.isLoadingMore) return;
  viewportHeight.value = el.clientHeight;
  let rounds = 0;
  while (
    rounds < 3 &&
    clipboardStore.hasMore &&
    !clipboardStore.isLoadingMore &&
    el.scrollHeight <= el.clientHeight + 40
  ) {
    rounds += 1;
    await clipboardStore.loadMore();
    await nextTick();
  }
}

// Explicit token from store after first-page load/search — not tied to isLoading churn.
watch(
  () => clipboardStore.viewportFillToken,
  () => {
    void fillViewportIfNeeded();
  }
);

onMounted(() => {
  void fillViewportIfNeeded();
});

const TYPE_LABELS: Record<string, string> = {
  text: '文本',
  code: '代码',
  link: '链接',
  image: '图片',
  file: '文件',
};

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

/** Layout-only row (no record payload — avoids rebuild on content/copy_count churn). */
interface FlatItem {
  key: string;
  type: "label" | "divider" | "record";
  id?: number;
  height: number;
  offset: number;
}

interface WindowItem extends FlatItem {
  record?: ClipboardRecord;
  thumb?: string | null;
}

/** Id order + pin flags + row heights — not content fields. */
const layoutSig = computed(() => {
  const records = clipboardStore.filteredRecords;
  const rh = rowHeight.value;
  const lh = labelHeight.value;
  const dh = dividerHeight.value;
  let sig = `${rh}|${lh}|${dh}|`;
  for (const r of records) {
    sig += `${r.id}:${r.is_pinned ? 1 : 0},`;
  }
  return sig;
});

function buildFlatItems(): FlatItem[] {
  const records = clipboardStore.filteredRecords;
  const items: FlatItem[] = [];
  let offset = 0;
  const rh = rowHeight.value;
  const lh = labelHeight.value;
  const dh = dividerHeight.value;
  const hasPinned = records.some((r) => r.is_pinned);
  const hasUnpinned = records.some((r) => !r.is_pinned);
  if (hasPinned) {
    items.push({ key: "pinned-label", type: "label", height: lh, offset });
    offset += lh;
  }
  let dividerInserted = false;
  for (const r of records) {
    if (hasPinned && hasUnpinned && !r.is_pinned && !dividerInserted) {
      items.push({ key: "pin-divider", type: "divider", height: dh, offset });
      offset += dh;
      dividerInserted = true;
    }
    items.push({
      key: `r-${r.id}`,
      type: "record",
      id: r.id,
      height: rh,
      offset,
    });
    offset += rh;
  }
  return items;
}

function buildRecordIndex(): Map<number, number> {
  const m = new Map<number, number>();
  clipboardStore.filteredRecords.forEach((r, i) => m.set(r.id, i));
  return m;
}

const flatItems = shallowRef<FlatItem[]>(buildFlatItems());
/** id → index in filteredRecords; rebuilt with layout only (not on content churn). */
const recordIndexById = shallowRef(buildRecordIndex());

watch(layoutSig, () => {
  flatItems.value = buildFlatItems();
  recordIndexById.value = buildRecordIndex();
});

const contentHeight = computed(() => {
  const items = flatItems.value;
  if (items.length === 0) return 0;
  const last = items[items.length - 1];
  return last.offset + last.height;
});

/** First index where item.offset + item.height >= target (item not fully above target). */
function lowerBoundPastTop(items: FlatItem[], target: number): number {
  let lo = 0;
  let hi = items.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (items[mid].offset + items[mid].height < target) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** First index where item.offset >= target. */
function lowerBoundByOffset(items: FlatItem[], target: number): number {
  let lo = 0;
  let hi = items.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (items[mid].offset < target) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

const virtualRange = computed(() => {
  const items = flatItems.value;
  const n = items.length;
  if (n === 0) return { start: 0, end: 0 };
  const top = scrollTop.value;
  const bottom = top + viewportHeight.value;
  let start = lowerBoundPastTop(items, top);
  let end = lowerBoundByOffset(items, bottom);
  start = Math.max(0, start - OVERSCAN);
  end = Math.min(n, end + OVERSCAN);
  return { start, end };
});

function resolveWindowItem(
  item: FlatItem,
  records: ClipboardRecord[],
  indexById: Map<number, number>
): WindowItem {
  if (item.type !== "record" || item.id == null) return item;
  const idx = indexById.get(item.id);
  const record = idx !== undefined ? records[idx] : undefined;
  if (!record) return item;
  return { ...item, record, thumb: recordThumbSrc(record) };
}

const windowItems = computed<WindowItem[]>(() => {
  const { start, end } = virtualRange.value;
  const slice = flatItems.value.slice(start, end);
  const records = clipboardStore.filteredRecords;
  const indexById = recordIndexById.value;
  return slice.map((item) => resolveWindowItem(item, records, indexById));
});

/** Grid: render all loaded rows (page size is small); list: virtual window. */
const displayItems = computed<WindowItem[]>(() => {
  if (listLayout.value !== "grid") return windowItems.value;
  const records = clipboardStore.filteredRecords;
  const indexById = recordIndexById.value;
  return flatItems.value.map((item) => resolveWindowItem(item, records, indexById));
});

const virtualPadTop = computed(() => {
  if (listLayout.value === "grid") return 0;
  const { start } = virtualRange.value;
  return start > 0 ? flatItems.value[start].offset : 0;
});

const virtualPadBottom = computed(() => {
  if (listLayout.value === "grid") return 0;
  const { end } = virtualRange.value;
  const items = flatItems.value;
  if (end >= items.length) return 0;
  return Math.max(0, contentHeight.value - items[end].offset);
});

const emptyState = computed(() => {
  if (clipboardStore.searchQuery) {
    return { icon: "search" as AppIconName, title: "没有找到匹配的结果", hint: "", clearSearch: true };
  }
  if (clipboardStore.trashFilter) {
    return { icon: "trash" as AppIconName, title: "回收站是空的", hint: "删除的记录会出现在这里", clearSearch: false };
  }
  if (clipboardStore.activeTag && clipboardStore.activeFilter !== "all") {
    const typeLabel =
      clipboardStore.activeFilter === "favorites"
        ? "收藏"
        : (TYPE_LABELS[clipboardStore.activeFilter] ?? clipboardStore.activeFilter);
    return {
      icon: "tag" as AppIconName,
      title: `${typeLabel} · ${clipboardStore.activeTag} 下暂无记录`,
      hint: "尝试取消其中一个筛选条件",
      clearSearch: false,
    };
  }
  if (clipboardStore.activeTag) {
    return { icon: "tag" as AppIconName, title: "该标签下暂无记录", hint: "可在预览区为记录添加标签", clearSearch: false };
  }
  if (clipboardStore.activeFilter === "favorites") {
    return { icon: "star" as AppIconName, title: "还没有收藏", hint: "按 Ctrl+D 或点击星标收藏记录", clearSearch: false };
  }
  if (clipboardStore.activeFilter !== "all") {
    const typeIconMap: Record<string, AppIconName> = {
      text: "type", code: "code", link: "link", image: "image", file: "file",
    };
    return {
      icon: typeIconMap[clipboardStore.activeFilter] ?? ("clipboard" as AppIconName),
      title: `暂无${TYPE_LABELS[clipboardStore.activeFilter] ?? ""}记录`,
      hint: "复制对应类型的内容后会出现在这里",
      clearSearch: false,
    };
  }
  return { icon: "clipboard" as AppIconName, title: "暂无剪贴板记录", hint: "复制任意内容即可开始使用", clearSearch: false };
});

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
      { id: "restore", label: "恢复", icon: "restore" },
      { id: "permanentDelete", label: "永久删除", icon: "trash", danger: true, separatorBefore: true },
    ];
  }
  const rec = contextMenu.record;
  return [
    { id: "paste", label: "粘贴", icon: "paste", shortcut: "Enter" },
    { id: "pastePlain", label: "纯文本粘贴", icon: "type", shortcut: "Alt+V" },
    {
      id: "favorite",
      label: rec?.is_favorite ? "取消收藏" : "收藏",
      icon: "star",
      shortcut: "Ctrl+D",
      separatorBefore: true,
    },
    {
      id: "pin",
      label: rec?.is_pinned ? "取消置顶" : "置顶",
      icon: "pin",
      shortcut: "Ctrl+T",
    },
    {
      id: "alias",
      label: rec?.alias?.trim() ? "编辑别名" : "设置别名",
      icon: "edit",
    },
    { id: "delete", label: "删除", icon: "trash", shortcut: "Del", danger: true, separatorBefore: true },
  ];
});

function recordAlias(record: ClipboardRecord): string {
  return (record.alias ?? "").trim();
}

function contentPreview(record: ClipboardRecord): string {
  if (record.content_type === "image") {
    if (record.width && record.height) {
      return `图片 ${record.width}×${record.height}`;
    }
    return "图片";
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
    toast("已粘贴", "success");
  } catch {
    toast("粘贴失败", "error");
  }
}

async function quickDelete(record: ClipboardRecord) {
  if (clipboardStore.trashFilter) {
    const ok = await confirm({
      title: "永久删除",
      message: "确定要永久删除这条记录吗？此操作不可恢复。",
      confirmText: "永久删除",
      danger: true,
    });
    if (ok) {
      await clipboardStore.permanentlyDeleteRecord(record.id);
      toast("已永久删除", "success");
    }
    return;
  }
  if (leavingIds.value.has(record.id)) return;
  const nextLeave = new Set(leavingIds.value);
  nextLeave.add(record.id);
  leavingIds.value = nextLeave;
  await sleep(160);
  await clipboardStore.deleteRecord(record.id);
  const cleared = new Set(leavingIds.value);
  cleared.delete(record.id);
  leavingIds.value = cleared;
  toast("已移到回收站", "success");
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
  if (diffMin < 1) return "刚刚";
  if (diffMin < 60) return `${diffMin} 分钟前`;
  if (diffMin < 1440) return `${Math.floor(diffMin / 60)} 小时前`;
  return d.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
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
    await clipboardStore.restoreRecord(id);
    return;
  }
  try {
    await clipboardStore.pasteRecord(id);
    toast("已粘贴", "success");
  } catch {
    toast("粘贴失败", "error");
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
      toast("已粘贴", "success");
    } catch {
      toast("粘贴失败", "error");
    }
    return;
  }
  if (id === "pastePlain") {
    try {
      await clipboardStore.pasteRecord(record.id, "plain");
      toast("已粘贴为纯文本", "success");
    } catch {
      toast("粘贴失败", "error");
    }
    return;
  }
  if (id === "favorite") {
    const next = await clipboardStore.toggleFavorite(record.id);
    if (next == null) toast("操作失败", "error");
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
      title: "永久删除",
      message: "确定要永久删除这条记录吗？此操作不可恢复。",
      confirmText: "永久删除",
      danger: true,
    });
    if (ok) {
      await clipboardStore.permanentlyDeleteRecord(record.id);
      toast("已永久删除", "success");
    }
  }
}

function closeContextMenu() {
  contextMenu.visible = false;
}

onMounted(() => {
  const el = listRef.value;
  if (el) {
    viewportHeight.value = el.clientHeight;
    scrollTop.value = el.scrollTop;
  }
});

onUnmounted(() => {
  if (scrollRaf) cancelAnimationFrame(scrollRaf);
});
</script>

<style scoped>
.record-list-wrapper {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.list-column {
  flex: 1.35;
  min-width: 280px;
  max-width: 520px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* Same surface as preview — sidebar stays elevated for nav hierarchy. */
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
}

.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  height: 44px;
  padding: 0 12px;
  flex-shrink: 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border-default) 60%, transparent);
}

.list-toolbar-left {
  display: flex;
  align-items: baseline;
  gap: 8px;
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
  gap: 6px;
  flex-shrink: 0;
  margin-left: auto;
}

.empty-trash-btn {
  height: 26px;
  padding: 0 8px;
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
  height: 26px;
  max-width: 7rem;
  padding: 0 6px;
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
  color: var(--accent);
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
  height: 26px;
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
  color: var(--accent);
  background: var(--accent-soft);
}

.view-toggle-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
  z-index: 1;
}

.record-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 0 6px;
}

/* —— Grid view: vertical cards (original structure) —— */
.record-list.view-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  padding: 10px;
  align-content: start;
}

.view-grid .section-label {
  grid-column: 1 / -1;
  padding: 4px 2px 0;
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
  padding: 10px;
  gap: 6px;
  /* Cap height so long text cannot blow out the grid track */
  height: 132px;
  max-height: 132px;
  box-sizing: border-box;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
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
  height: 140px;
  max-height: 140px;
}

.view-grid .record-item.batch-mode {
  padding: 10px;
}

.view-grid .record-checkbox {
  left: auto;
  right: 8px;
  top: 8px;
  z-index: 3;
  width: 18px;
  height: 18px;
  border-radius: 5px;
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
  gap: 4px;
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
  top: 6px;
  right: 6px;
  margin: 0;
  z-index: 2;
  max-width: calc(100% - 12px);
  overflow: hidden;
  background: color-mix(in srgb, var(--bg-surface) 94%, transparent);
  border-radius: 6px;
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
  padding: 4px 0 8px;
}

.virtual-spacer {
  width: 100%;
  flex-shrink: 0;
  pointer-events: none;
}

.section-label {
  font-size: var(--text-xs, 0.625rem);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--pin);
  padding: 10px 14px 4px;
  display: flex;
  align-items: center;
  gap: 4px;
}

.pin-section-divider {
  box-sizing: border-box;
  flex-shrink: 0;
  width: 100%;
  margin: 0;
  padding: 0 14px;
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
  padding: 10px 12px;
  margin: 0 4px 2px;
  cursor: pointer;
  border-radius: 6px;
  transition:
    background var(--transition-fast),
    opacity var(--transition-fast),
    transform var(--transition-fast);
  display: flex;
  align-items: flex-start;
  gap: 10px;
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
  font-size: 0.563rem;
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

.record-type-icon.text {
  background: color-mix(in srgb, var(--type-text) 16%, transparent);
  color: var(--type-text);
}

.record-type-icon.code {
  background: color-mix(in srgb, var(--type-code) 16%, transparent);
  color: var(--type-code);
}

.record-type-icon.link {
  background: color-mix(in srgb, var(--type-link) 16%, transparent);
  color: var(--type-link);
}

.record-type-icon.image {
  background: color-mix(in srgb, var(--type-image) 16%, transparent);
  color: var(--type-image);
}

.record-type-icon.file {
  background: color-mix(in srgb, var(--type-file) 16%, transparent);
  color: var(--type-file);
}

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
  gap: 8px;
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
  border-radius: 6px;
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
  color: var(--accent);
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
  padding: 10px 16px 14px;
  text-align: center;
  font-size: 0.719rem;
  color: var(--text-muted, var(--text-tertiary));
  border-top: 1px solid var(--border-light, var(--border-subtle));
  margin-top: 4px;
}

/* Empty / Loading */
.loading-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--text-tertiary);
  font-size: 0.75rem;
  flex: 1;
  padding: 20px;
  text-align: center;
}

.loading-spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--border-default);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

.loading-spinner.small {
  width: 13px;
  height: 13px;
  border-width: 1.5px;
}

.footer-loading {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.empty-icon {
  width: 48px;
  height: 48px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-tertiary);
  opacity: 0.9;
  margin-bottom: 4px;
}

.empty-text {
  font-size: 0.813rem;
}

.empty-hint {
  font-size: 0.688rem;
  color: var(--text-tertiary);
}

.clear-link {
  color: var(--accent);
  cursor: pointer;
  text-decoration: underline;
}
</style>
