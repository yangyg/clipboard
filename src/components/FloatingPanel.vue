<template>
  <div class="floating-panel panel-surface">
    <!-- Header -->
    <div class="panel-header">
      <div class="panel-title-row">
        <div>
          <div class="panel-title">剪贴板管理</div>
          <CaptureStatus compact />
        </div>
      </div>
      <SearchBar />
      <div class="header-actions">
        <button
          class="icon-btn"
          :class="{ active: clipboardStore.batchMode }"
          title="批量操作"
          @click="toggleBatchMode"
        ><AppIcon name="batch" :size="15" /></button>
        <button
          class="icon-btn"
          :class="{ active: clipboardStore.activeFilter === 'favorites' }"
          title="收藏"
          @click="clipboardStore.setFilter(clipboardStore.activeFilter === 'favorites' ? 'all' : 'favorites')"
        ><AppIcon name="star" :size="15" :fill="clipboardStore.activeFilter === 'favorites' ? 'currentColor' : 'none'" /></button>
        <button
          class="icon-btn"
          :class="{ active: clipboardStore.trashFilter }"
          title="回收站"
          @click="toggleTrash"
        ><AppIcon name="trash" :size="15" /></button>
        <button class="icon-btn" title="设置" @click="emit('openSettings')"><AppIcon name="settings" :size="15" /></button>
      </div>
    </div>

    <!-- Filter Tabs (hidden in trash) -->
    <div v-if="!clipboardStore.trashFilter" class="filter-row">
      <button
        v-for="tab in FILTER_TABS"
        :key="tab.key"
        class="filter-tab"
        :class="{ active: clipboardStore.activeFilter === tab.key }"
        @click="clipboardStore.setFilter(tab.key)"
      >
        {{ tab.label }}
        <span class="filter-count">{{ clipboardStore.filterCounts[tab.key] }}</span>
      </button>
    </div>
    <div v-else class="trash-banner">
      <span>回收站 · {{ clipboardStore.trashCount }} 项</span>
      <button
        v-if="clipboardStore.trashCount > 0"
        class="empty-trash-link"
        @click="onEmptyTrash"
      >清空</button>
    </div>

    <!-- Body -->
    <div class="panel-body">
      <Transition name="fade">
        <div v-if="clipboardStore.batchMode" class="batch-bar">
          <div class="batch-info">
            <AppIcon name="batch" :size="13" /> 已选择 <strong>{{ clipboardStore.selectedIds.size }}</strong> 项
          </div>
          <div class="batch-actions">
            <button class="batch-btn" @click="batchCopy"><AppIcon name="copy" :size="13" /> 复制</button>
            <button class="batch-btn" @click="batchFavorite"><AppIcon name="star" :size="13" /> 收藏</button>
            <button class="batch-btn danger-btn" @click="batchDelete"><AppIcon name="trash" :size="13" /> 删除</button>
            <button class="batch-btn" @click="toggleBatchMode"><AppIcon name="close" :size="13" /></button>
          </div>
        </div>
      </Transition>

      <RecordList />
    </div>
  </div>
</template>

<script setup lang="ts">
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import CaptureStatus from "./CaptureStatus.vue";
import AppIcon from "./icons/AppIcon.vue";
import { useClipboardStore } from "../stores/clipboard";
import type { FilterTab } from "../stores/clipboard";
import { useClipboardHotkeys } from "../composables/useClipboardHotkeys";
import { useBatchActions } from "../composables/useBatchActions";
import { useConfirm } from "../composables/useConfirm";
import { useToast } from "../composables/useToast";

const emit = defineEmits<{
  openSettings: [];
  close: [];
}>();

const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { toggleBatchMode, batchCopy, batchFavorite, batchDelete } = useBatchActions();

useClipboardHotkeys({
  onClose: () => emit("close"),
  allowCloseOnEscape: true,
});

const FILTER_TABS: { key: FilterTab; label: string }[] = [
  { key: "all", label: "全部" },
  { key: "text", label: "文本" },
  { key: "code", label: "代码" },
  { key: "link", label: "链接" },
  { key: "image", label: "图片" },
  { key: "file", label: "文件" },
];

async function toggleTrash() {
  const next = !clipboardStore.trashFilter;
  clipboardStore.setTrashFilter(next);
  if (next) {
    await clipboardStore.loadRecords();
  } else {
    await clipboardStore.search("");
  }
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
</script>

<style scoped>
.floating-panel {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-header {
  padding: 12px 14px 8px;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex-shrink: 0;
  position: relative;
}

.panel-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-right: 130px;
}

.panel-title {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--text-primary);
}

.header-actions {
  display: flex;
  gap: 2px;
  position: absolute;
  top: 12px;
  right: 14px;
}

.filter-row {
  display: flex;
  gap: 2px;
  padding: 0 14px 8px;
  flex-shrink: 0;
  overflow-x: auto;
}

.filter-tab {
  height: 26px;
  padding: 0 10px;
  border-radius: 5px;
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--text-secondary);
  transition: all var(--transition-fast);
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
}

.filter-tab:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.filter-tab.active {
  background: var(--accent-soft);
  color: var(--accent);
}

.filter-count {
  font-size: 0.625rem;
  background: var(--bg-active);
  padding: 1px 5px;
  border-radius: 4px;
  color: var(--text-tertiary);
}

.filter-tab.active .filter-count {
  background: var(--accent-soft);
  color: var(--accent);
}

.trash-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 14px 10px;
  font-size: 0.72rem;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border-subtle);
}

.empty-trash-link {
  color: var(--danger);
  font-weight: 500;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.empty-trash-link:hover {
  background: var(--danger-soft);
}

.panel-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  position: relative;
}

.batch-bar {
  padding: 8px 14px;
  background: var(--accent-soft);
  border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, transparent);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.batch-info {
  font-size: 0.72rem;
  color: var(--accent);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
}

.batch-actions {
  display: flex;
  gap: 6px;
}

.batch-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: 0.69rem;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--bg-surface);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  transition: all var(--transition-fast);
  cursor: pointer;
}

.batch-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.batch-btn.danger-btn {
  background: var(--danger-soft);
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 20%, transparent);
}

.batch-btn.danger-btn:hover {
  background: color-mix(in srgb, var(--danger) 20%, transparent);
}
</style>
