<template>
  <div class="record-list-wrapper">
    <!-- Loading -->
    <div v-if="clipboardStore.isLoading" class="loading-state">
      <div class="loading-spinner"></div>
      <span>加载中…</span>
    </div>

    <!-- Empty -->
    <div v-else-if="clipboardStore.filteredRecords.length === 0" class="empty-state">
      <div class="empty-icon">{{ clipboardStore.searchQuery ? '🔍' : '📋' }}</div>
      <div class="empty-text">
        {{ clipboardStore.searchQuery ? '没有找到匹配的结果' : '暂无剪贴板记录' }}
      </div>
      <div v-if="clipboardStore.searchQuery" class="empty-hint">
        试试其他关键词，或
        <button class="clear-link" @click="clipboardStore.search('')">清除搜索</button>
      </div>
    </div>

    <!-- Record List -->
    <div v-else class="record-list" ref="listRef">
      <template v-for="item in visibleItems" :key="item.type === 'label' ? 'pinned-label' : item.record!.id">
        <div v-if="item.type === 'label'" class="section-label">📌 置顶</div>
        <div
          v-else
          class="record-item"
          :class="{ selected: clipboardStore.selectedId === item.record!.id, 'batch-mode': clipboardStore.batchMode }"
          @click="onItemClick(item.record!.id)"
          @dblclick="onItemDoubleClick(item.record!.id)"
          @contextmenu.prevent="showContextMenu($event, item.record!)"
        >
          <div v-if="clipboardStore.batchMode" class="record-checkbox" :class="{ checked: clipboardStore.selectedIds.has(item.record!.id) }">
            <span v-if="clipboardStore.selectedIds.has(item.record!.id)">✓</span>
          </div>
          <div class="record-type-icon" :class="item.record!.content_type">
            {{ typeIcon(item.record!.content_type) }}
          </div>
          <div class="record-body">
            <div class="record-title">{{ getPreview(item.record!) }}</div>
            <div class="record-meta">
              <span class="record-time">{{ formatTime(item.record!.created_at) }}</span>
              <span class="record-badge" :class="badgeClass(item.record!)">
                {{ TYPE_LABELS[item.record!.content_type] || '文本' }}
              </span>
              <span class="record-chars" v-if="isTextLike(item.record!.content_type)">· {{ item.record!.content.length }} 字符</span>
            </div>
          </div>
          <div class="record-actions">
            <button
              class="record-pin"
              :class="{ pinned: item.record!.is_pinned }"
              @click.stop="clipboardStore.togglePin(item.record!.id)"
              :title="item.record!.is_pinned ? '取消置顶' : '置顶'"
            >📌</button>
            <button
              class="record-star"
              :class="{ starred: item.record!.is_favorite }"
              @click.stop="clipboardStore.toggleFavorite(item.record!.id)"
              :title="item.record!.is_favorite ? '取消收藏' : '收藏'"
            >★</button>
          </div>
        </div>
      </template>

      <!-- Footer -->
      <div class="list-footer">
        {{ clipboardStore.filteredRecords.length }} 项内容
      </div>
    </div>

    <!-- Preview Pane (right side) -->
    <PreviewPane v-if="clipboardStore.selectedRecord && !clipboardStore.batchMode" />

    <!-- Context Menu -->
    <div
      v-if="contextMenu.visible"
      class="context-menu"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
      @click.stop
    >
      <template v-if="!clipboardStore.trashFilter">
        <div class="ctx-item" @click="ctxPaste">
          <span class="ctx-icon">⏎</span>粘贴
          <span class="ctx-shortcut">Enter</span>
        </div>
        <div class="ctx-item" @click="ctxPastePlain">
          <span class="ctx-icon">Aa</span>纯文本粘贴
          <span class="ctx-shortcut">Alt+V</span>
        </div>
        <div class="ctx-sep"></div>
        <div class="ctx-item" @click="ctxFavorite">
          <span class="ctx-icon">★</span>{{ contextMenu.record?.is_favorite ? '取消收藏' : '收藏' }}
          <span class="ctx-shortcut">Ctrl+D</span>
        </div>
        <div class="ctx-item" @click="ctxPin">
          <span class="ctx-icon">📌</span>{{ contextMenu.record?.is_pinned ? '取消置顶' : '置顶' }}
          <span class="ctx-shortcut">Ctrl+T</span>
        </div>
        <div class="ctx-sep"></div>
        <div class="ctx-item danger" @click="ctxDelete">
          <span class="ctx-icon">🗑</span>删除
          <span class="ctx-shortcut">Del</span>
        </div>
      </template>
      <template v-else>
        <div class="ctx-item" @click="ctxRestore">
          <span class="ctx-icon">↩</span>恢复
        </div>
        <div class="ctx-sep"></div>
        <div class="ctx-item danger" @click="ctxPermanentDelete">
          <span class="ctx-icon">🗑</span>永久删除
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, onMounted, onUnmounted } from "vue";
import { useClipboardStore } from "../stores/clipboard";
import PreviewPane from "./PreviewPane.vue";
import type { ClipboardRecord, ContentType } from "../types";

const clipboardStore = useClipboardStore();

const TYPE_LABELS: Record<ContentType, string> = {
  text: '文本',
  code: '代码',
  link: '链接',
  image: '图片',
  file: '文件',
  sensitive: '敏感',
};

const TYPE_ICONS: Record<string, string> = {
  text: 'Aa',
  code: '</>',
  link: '🔗',
  image: '🖼',
  file: '📄',
};

function typeIcon(type: ContentType): string {
  return TYPE_ICONS[type] ?? 'T';
}

function isTextLike(type: string): boolean {
  return type === 'text' || type === 'code';
}

interface VisibleItem {
  type: "label" | "record";
  record?: ClipboardRecord;
}

const visibleItems = computed<VisibleItem[]>(() => {
  const items: VisibleItem[] = [];
  const records = clipboardStore.filteredRecords;
  const pinned = records.filter((r) => r.is_pinned);
  const regular = records.filter((r) => !r.is_pinned);
  if (pinned.length > 0) {
    items.push({ type: "label" });
    for (const r of pinned) items.push({ type: "record", record: r });
  }
  for (const r of regular) items.push({ type: "record", record: r });
  return items;
});

const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  record: null as ClipboardRecord | null,
});

function getPreview(record: ClipboardRecord): string {
  const maxLen = 80;
  if (record.content.length <= maxLen) return record.content;
  return record.content.slice(0, maxLen) + "…";
}

function badgeClass(record: ClipboardRecord): string {
  if (record.is_sensitive) return "badge-sensitive";
  return `badge-${record.content_type}`;
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
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

function onItemDoubleClick(id: number) {
  if (clipboardStore.trashFilter) {
    clipboardStore.restoreRecord(id);
  } else {
    clipboardStore.pasteRecord(id);
  }
}

function showContextMenu(e: MouseEvent, record: ClipboardRecord) {
  const wrapper = (e.currentTarget as HTMLElement).closest(".record-list-wrapper")?.getBoundingClientRect();
  contextMenu.visible = true;
  contextMenu.x = wrapper ? e.clientX - wrapper.left : e.offsetX;
  contextMenu.y = wrapper ? Math.min(e.clientY - wrapper.top, wrapper.height - 210) : Math.min(e.offsetY, 300);
  contextMenu.record = record;
}

function ctxPaste() {
  if (contextMenu.record) clipboardStore.pasteRecord(contextMenu.record.id);
  contextMenu.visible = false;
}

function ctxPastePlain() {
  if (contextMenu.record) clipboardStore.pasteRecord(contextMenu.record.id, "plain");
  contextMenu.visible = false;
}

function ctxFavorite() {
  if (contextMenu.record) clipboardStore.toggleFavorite(contextMenu.record.id);
  contextMenu.visible = false;
}

function ctxPin() {
  if (contextMenu.record) clipboardStore.togglePin(contextMenu.record.id);
  contextMenu.visible = false;
}

function ctxDelete() {
  if (contextMenu.record) clipboardStore.deleteRecord(contextMenu.record.id);
  contextMenu.visible = false;
}

function ctxRestore() {
  if (contextMenu.record) clipboardStore.restoreRecord(contextMenu.record.id);
  contextMenu.visible = false;
}

function ctxPermanentDelete() {
  if (contextMenu.record && confirm("确定要永久删除这条记录吗？此操作不可恢复。")) {
    clipboardStore.permanentlyDeleteRecord(contextMenu.record.id);
  }
  contextMenu.visible = false;
}

function closeContextMenu() {
  contextMenu.visible = false;
}

onMounted(() => {
  window.addEventListener("click", closeContextMenu);
});

onUnmounted(() => {
  window.removeEventListener("click", closeContextMenu);
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

.section-label {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
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
  border-color: rgba(79, 110, 247, 0.2);
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
  font-size: 9px;
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
  font-size: 13px;
  font-weight: 600;
  flex-shrink: 0;
  margin-top: 1px;
}

.record-type-icon.text {
  background: rgba(79, 110, 247, 0.1);
  color: var(--accent);
}

.record-type-icon.code {
  background: rgba(124, 92, 252, 0.1);
  color: #7c5cfc;
}

.record-type-icon.link {
  background: rgba(23, 192, 146, 0.1);
  color: #17a97b;
}

.record-type-icon.image {
  background: rgba(232, 125, 62, 0.1);
  color: #e87d3e;
}

.record-type-icon.file {
  background: rgba(232, 106, 51, 0.1);
  color: #e86a33;
}

.record-body {
  flex: 1;
  min-width: 0;
}

.record-title {
  font-size: 13px;
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
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.record-time {
  white-space: nowrap;
}

.record-chars {
  white-space: nowrap;
}

.record-badge {
  font-size: 10.5px;
  font-weight: 600;
  padding: 1px 7px;
  border-radius: 4px;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.badge-text {
  background: rgba(79, 110, 247, 0.1);
  color: var(--accent);
}

.badge-code {
  background: rgba(124, 92, 252, 0.1);
  color: #7c5cfc;
}

.badge-link {
  background: rgba(23, 192, 146, 0.1);
  color: #17a97b;
}

.badge-image {
  background: rgba(232, 125, 62, 0.1);
  color: #e87d3e;
}

.badge-file {
  background: rgba(232, 106, 51, 0.1);
  color: #e86a33;
}

.badge-sensitive {
  background: var(--danger-soft);
  color: var(--danger);
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
  font-size: 14px;
  background: none;
  border: none;
  cursor: pointer;
  opacity: 0;
  transition: opacity var(--transition-fast), transform var(--transition-fast);
  line-height: 1;
  padding: 1px 2px;
  color: var(--text-muted, var(--text-tertiary));
}

.record-star {
  font-size: 17px;
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
  font-size: 11.5px;
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
  font-size: 12px;
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
  width: 42px;
  height: 42px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  font-size: 22px;
  opacity: 0.4;
}

.empty-text {
  font-size: 13px;
}

.empty-hint {
  font-size: 11px;
  color: var(--text-tertiary);
}

.clear-link {
  color: var(--accent);
  cursor: pointer;
  text-decoration: underline;
}

/* Context Menu */
.context-menu {
  position: absolute;
  width: 190px;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  padding: 6px;
  z-index: 100;
}

.ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  font-size: 12px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.ctx-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.ctx-item.danger {
  color: var(--danger);
}

.ctx-item.danger:hover {
  background: var(--danger-soft);
}

.ctx-icon {
  width: 16px;
  text-align: center;
  font-size: 12px;
}

.ctx-shortcut {
  margin-left: auto;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-tertiary);
}

.ctx-sep {
  height: 1px;
  background: var(--border-subtle);
  margin: 3px 6px;
}
</style>
