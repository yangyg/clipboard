<template>
  <div class="record-list-wrapper">
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
      ref="listRef"
      role="listbox"
      aria-label="剪贴板记录"
      :aria-activedescendant="activeDescendantId"
      tabindex="-1"
      @scroll="onListScroll"
    >
      <div class="virtual-spacer" :style="{ height: `${virtualPadTop}px` }" aria-hidden="true"></div>
      <template v-for="item in windowItems" :key="item.key">
        <div v-if="item.type === 'label'" class="section-label" aria-hidden="true"><AppIcon name="pin" :size="11" /> 置顶</div>
        <div
          v-else
          :id="`record-option-${item.record!.id}`"
          class="record-item"
          role="option"
          :aria-selected="clipboardStore.selectedId === item.record!.id"
          :tabindex="isOptionTabbable(item.record!.id) ? 0 : -1"
          :class="{ selected: clipboardStore.selectedId === item.record!.id, 'batch-mode': clipboardStore.batchMode }"
          :data-record-id="item.record!.id"
          @click="onItemClick(item.record!.id)"
          @dblclick="onItemDoubleClick(item.record!.id)"
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
          <div
            class="record-type-icon"
            :class="[item.record!.content_type, { 'has-thumb': !!item.thumb }]"
            aria-hidden="true"
          >
            <img
              v-if="item.thumb"
              class="record-thumb"
              :src="item.thumb"
              alt=""
            />
            <TypeIcon v-else :type="item.record!.content_type" :size="13" />
          </div>
          <div class="record-body">
            <div class="record-title">{{ getPreview(item.record!) }}</div>
            <div class="record-meta">
              <span class="record-time">{{ formatTime(item.record!.created_at) }}</span>
              <span class="record-badge" :class="badgeClass(item.record!)">
                {{ TYPE_LABELS[item.record!.content_type] || '文本' }}
              </span>
              <span class="record-chars" v-if="isTextLike(item.record!.content_type)">· {{ item.record!.content_len ?? item.record!.content.length }} 字符</span>
              <span class="record-chars" v-else-if="item.record!.content_type === 'image' && item.record!.width && item.record!.height">
                · {{ item.record!.width }}×{{ item.record!.height }}
              </span>
            </div>
          </div>
          <div class="record-actions">
            <button
              type="button"
              class="record-pin"
              :class="{ pinned: item.record!.is_pinned }"
              :aria-label="item.record!.is_pinned ? '取消置顶' : '置顶'"
              :title="item.record!.is_pinned ? '取消置顶' : '置顶'"
              @click.stop="clipboardStore.togglePin(item.record!.id)"
            ><AppIcon name="pin" :size="13" :fill="item.record!.is_pinned ? 'currentColor' : 'none'" /></button>
            <button
              type="button"
              class="record-star"
              :class="{ starred: item.record!.is_favorite }"
              :aria-label="item.record!.is_favorite ? '取消收藏' : '收藏'"
              :title="item.record!.is_favorite ? '取消收藏' : '收藏'"
              @click.stop="clipboardStore.toggleFavorite(item.record!.id)"
            ><AppIcon name="star" :size="13" :fill="item.record!.is_favorite ? 'currentColor' : 'none'" /></button>
          </div>
        </div>
      </template>
      <div class="virtual-spacer" :style="{ height: `${virtualPadBottom}px` }" aria-hidden="true"></div>

      <!-- Footer: load-more status only -->
      <div v-if="clipboardStore.isLoadingMore || clipboardStore.hasMore" class="list-footer">
        <span v-if="clipboardStore.isLoadingMore">加载更多…</span>
        <span v-else>继续滚动加载更多</span>
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
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch, nextTick, onMounted, onUnmounted, shallowRef } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import PreviewPane from "./PreviewPane.vue";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu.vue";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import TypeIcon from "./icons/TypeIcon.vue";
import type { ClipboardRecord } from "../types";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";
import { recordThumbSrc } from "../utils/mediaUrl";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const listRef = ref<HTMLElement | null>(null);

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
const BASE_ROW_HEIGHT = 58;
const BASE_LABEL_HEIGHT = 26;
const OVERSCAN = 6;
const rowHeight = computed(() =>
  Math.round(BASE_ROW_HEIGHT * (settingsStore.settings.font_size / 16))
);
const labelHeight = computed(() =>
  Math.round(BASE_LABEL_HEIGHT * (settingsStore.settings.font_size / 16))
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

function isTextLike(type: string): boolean {
  return type === 'text' || type === 'code';
}

/** Layout-only row (no record payload — avoids rebuild on content/copy_count churn). */
interface FlatItem {
  key: string;
  type: "label" | "record";
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
  let sig = `${rh}|${lh}|`;
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
  const hasPinned = records.some((r) => r.is_pinned);
  if (hasPinned) {
    items.push({ key: "pinned-label", type: "label", height: lh, offset });
    offset += lh;
  }
  for (const r of records) {
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

const flatItems = shallowRef<FlatItem[]>(buildFlatItems());

watch(
  layoutSig,
  () => {
    flatItems.value = buildFlatItems();
  }
);

const contentHeight = computed(() => {
  const items = flatItems.value;
  if (items.length === 0) return 0;
  const last = items[items.length - 1];
  return last.offset + last.height;
});

const virtualRange = computed(() => {
  const items = flatItems.value;
  const n = items.length;
  if (n === 0) return { start: 0, end: 0 };
  const top = scrollTop.value;
  const bottom = top + viewportHeight.value;
  let start = 0;
  while (start < n && items[start].offset + items[start].height < top) start += 1;
  let end = start;
  while (end < n && items[end].offset < bottom) end += 1;
  start = Math.max(0, start - OVERSCAN);
  end = Math.min(n, end + OVERSCAN);
  return { start, end };
});

/** Resolve live records for the visible window only (O(visible) lookups). */
const recordsById = computed(() => {
  const m = new Map<number, ClipboardRecord>();
  for (const r of clipboardStore.filteredRecords) m.set(r.id, r);
  return m;
});

const windowItems = computed<WindowItem[]>(() => {
  const { start, end } = virtualRange.value;
  const slice = flatItems.value.slice(start, end);
  const byId = recordsById.value;
  return slice.map((item) => {
    if (item.type !== "record" || item.id == null) return item;
    const record = byId.get(item.id);
    if (!record) return item;
    return { ...item, record, thumb: recordThumbSrc(record) };
  });
});

const virtualPadTop = computed(() => {
  const { start } = virtualRange.value;
  return start > 0 ? flatItems.value[start].offset : 0;
});

const virtualPadBottom = computed(() => {
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
    { id: "delete", label: "删除", icon: "trash", shortcut: "Del", danger: true, separatorBefore: true },
  ];
});

function getPreview(record: ClipboardRecord): string {
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

function badgeClass(record: ClipboardRecord): string {
  if (record.is_sensitive) return "badge-sensitive";
  return `badge-${record.content_type}`;
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

async function onItemActivate(id: number) {
  if (clipboardStore.batchMode) {
    clipboardStore.toggleBatchSelect(id);
    return;
  }
  await onItemDoubleClick(id);
}

async function onItemDoubleClick(id: number) {
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
    const next = await clipboardStore.togglePin(record.id);
    if (next == null) toast("操作失败", "error");
    return;
  }
  if (id === "restore") {
    await clipboardStore.restoreRecord(record.id);
    return;
  }
  if (id === "delete") {
    await clipboardStore.deleteRecord(record.id);
    toast("已移到回收站", "success");
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

.record-list {
  flex: 1;
  min-width: 240px;
  max-width: 400px;
  overflow-y: auto;
}

.virtual-spacer {
  width: 100%;
  flex-shrink: 0;
  pointer-events: none;
}

.section-label {
  font-size: 0.625rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-tertiary);
  padding: 8px 16px 2px;
}

.record-item {
  padding: 10px 12px;
  margin: 0 8px 2px;
  cursor: pointer;
  border-radius: var(--radius-md, 10px);
  transition: background var(--transition-fast), border-color var(--transition-fast);
  display: flex;
  align-items: flex-start;
  gap: 10px;
  position: relative;
  border: 1px solid transparent;
}

.record-item:hover {
  background: var(--bg-hover);
}

.record-item:hover .record-star {
  opacity: 1;
}

.record-item.selected {
  background: var(--bg-selected, var(--accent-soft));
  border-color: color-mix(in srgb, var(--accent) 20%, transparent);
}

.record-item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.record-item.selected::before {
  content: "";
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 2px;
  background: var(--accent);
  border-radius: 0 2px 2px 0;
}

.record-item.batch-mode {
  padding-left: 30px;
}

.record-checkbox {
  position: absolute;
  left: 8px;
  top: 13px;
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

/* Type Icon */
.record-type-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.813rem;
  font-weight: 600;
  flex-shrink: 0;
  margin-top: 1px;
  overflow: hidden;
}

.record-type-icon.has-thumb {
  background: var(--bg-surface);
  padding: 0;
}

.record-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.record-type-icon.text {
  background: color-mix(in srgb, var(--type-text) 15%, transparent);
  color: var(--type-text);
}

.record-type-icon.code {
  background: color-mix(in srgb, var(--type-code) 15%, transparent);
  color: var(--type-code);
}

.record-type-icon.link {
  background: color-mix(in srgb, var(--type-link) 15%, transparent);
  color: var(--type-link);
}

.record-type-icon.image {
  background: color-mix(in srgb, var(--type-image) 15%, transparent);
  color: var(--type-image);
}

.record-type-icon.file {
  background: color-mix(in srgb, var(--type-file) 15%, transparent);
  color: var(--type-file);
}

.record-body {
  flex: 1;
  min-width: 0;
}

.record-title {
  font-size: 0.813rem;
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.record-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 5px;
  font-size: 0.719rem;
  color: var(--text-tertiary);
}

.record-time {
  white-space: nowrap;
}

.record-chars {
  white-space: nowrap;
}

.record-badge {
  font-size: 0.656rem;
  font-weight: 600;
  padding: 1px 7px;
  border-radius: 4px;
}

/* Pin + Star buttons - hidden by default, shows on row hover */
.record-actions {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  margin-top: 1px;
}

.record-pin,
.record-star {
  font-size: 0.875rem;
  background: none;
  border: none;
  cursor: pointer;
  opacity: 0.35;
  transition: opacity var(--transition-fast), transform var(--transition-fast);
  line-height: 1;
  padding: 1px 2px;
  color: var(--text-muted, var(--text-tertiary));
}

.record-pin:focus-visible,
.record-star:focus-visible {
  opacity: 1;
  outline: 1px solid var(--accent);
  border-radius: 4px;
}

.record-star {
  font-size: 1.063rem;
}

.record-pin.pinned {
  opacity: 1;
}

.record-star.starred {
  opacity: 1;
  color: var(--warning);
}

.record-pin:hover,
.record-star:hover {
  transform: scale(1.2);
}

.record-item:hover .record-pin,
.record-item:hover .record-star {
  opacity: 0.6;
}

.record-item:hover .record-pin.pinned {
  opacity: 1;
}

.record-pin:hover {
  opacity: 1 !important;
}

.record-star:hover {
  opacity: 1 !important;
  color: var(--warning);
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
