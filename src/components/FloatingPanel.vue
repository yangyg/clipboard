<template>
  <div class="floating-panel">
    <!-- Header -->
    <div class="panel-header">
      <div class="panel-title-row">
        <div>
          <div class="panel-title">ClipVault</div>
          <CaptureStatus compact />
        </div>
      </div>
      <SearchBar />
      <div class="header-actions">
        <button
          class="icon-btn"
          :class="{ active: clipboardStore.batchMode }"
          title="批量操作"
          @click="clipboardStore.toggleBatchMode()"
        >☐</button>
        <button
          class="icon-btn"
          :class="{ active: clipboardStore.activeFilter === 'favorites' }"
          title="收藏"
          @click="clipboardStore.setFilter(clipboardStore.activeFilter === 'favorites' ? 'all' : 'favorites')"
        >★</button>
        <button class="icon-btn" title="设置" @click="emit('openSettings')">⚙</button>
      </div>
    </div>

    <!-- Filter Tabs -->
    <div class="filter-row">
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

    <div class="stats-row" v-if="clipboardStore.stats">
      <div class="stat-pill">
        <span class="stat-value">{{ clipboardStore.stats.total_records }}</span>
        <span class="stat-label">条记录</span>
      </div>
      <div class="stat-pill">
        <span class="stat-value">{{ clipboardStore.stats.total_copies }}</span>
        <span class="stat-label">次复制</span>
      </div>
      <div class="stat-pill warning">
        <span class="stat-value">{{ clipboardStore.stats.favorites_count }}</span>
        <span class="stat-label">收藏</span>
      </div>
      <div class="stat-pill sensitive">
        <span class="stat-value">{{ clipboardStore.stats.sensitive_count }}</span>
        <span class="stat-label">敏感</span>
      </div>
    </div>

    <!-- Body -->
    <div class="panel-body">
      <!-- Batch bar -->
      <Transition name="fade">
        <div v-if="clipboardStore.batchMode" class="batch-bar">
          <div class="batch-info">
            <span>☑</span> 已选择 <strong>{{ clipboardStore.selectedIds.size }}</strong> 项
          </div>
          <div class="batch-actions">
            <button class="batch-btn" @click="batchCopy">📋 复制</button>
            <button class="batch-btn" @click="batchFavorite">★ 收藏</button>
            <button class="batch-btn danger-btn" @click="batchDelete">🗑 删除</button>
            <button class="batch-btn" @click="clipboardStore.toggleBatchMode()">✕</button>
          </div>
        </div>
      </Transition>

      <!-- Record List + Preview -->
      <RecordList />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import SearchBar from "./SearchBar.vue";
import RecordList from "./RecordList.vue";
import CaptureStatus from "./CaptureStatus.vue";
import { useClipboardStore } from "../stores/clipboard";
import type { FilterTab } from "../stores/clipboard";

const emit = defineEmits<{
  openSettings: [];
  close: [];
}>();

const clipboardStore = useClipboardStore();

const FILTER_TABS: { key: FilterTab; label: string }[] = [
  { key: "all", label: "全部" },
  { key: "text", label: "文本" },
  { key: "code", label: "代码" },
  { key: "link", label: "链接" },
  { key: "image", label: "图片" },
  { key: "file", label: "文件" },
];

// Keyboard navigation
function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (clipboardStore.batchMode) {
      clipboardStore.toggleBatchMode();
      return;
    }
    if (clipboardStore.selectedId !== null) {
      clipboardStore.selectRecord(clipboardStore.selectedId);
      return;
    }
    // Close panel on Escape
    emit("close");
    return;
  }

  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const list = clipboardStore.filteredRecords;
    if (!list.length) return;
    const currentIdx = list.findIndex((r) => r.id === clipboardStore.selectedId);
    let nextIdx = e.key === "ArrowDown"
      ? Math.min(currentIdx + 1, list.length - 1)
      : Math.max(currentIdx - 1, 0);
    if (currentIdx === -1) nextIdx = 0;
    clipboardStore.selectRecord(list[nextIdx].id);
  }

  if (e.key === "Enter") {
    if (clipboardStore.selectedId !== null) {
      clipboardStore.pasteRecord(clipboardStore.selectedId);
    }
  }
}

async function batchCopy() {
  const ids = Array.from(clipboardStore.selectedIds);
  if (!ids.length) return;
  const selected = clipboardStore.records.filter((record) => ids.includes(record.id));
  if (selected.length) {
    await navigator.clipboard.writeText(selected.map((record) => record.content).join("\n\n"));
  }
}

async function batchFavorite() {
  for (const id of clipboardStore.selectedIds) {
    await clipboardStore.toggleFavorite(id);
  }
}

async function batchDelete() {
  const ids = Array.from(clipboardStore.selectedIds);
  await clipboardStore.deleteBatch(ids);
}

onMounted(() => {
  window.addEventListener("keydown", onKeyDown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
});
</script>

<style scoped>
.floating-panel {
  width: 100%;
  height: 100%;
  background: color-mix(in srgb, var(--bg-surface) calc(var(--panel-opacity, 0.94) * 100%), transparent);
  border-radius: var(--panel-radius, 20px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: background var(--transition-smooth);
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
  padding-right: 100px;
}

.panel-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: 0;
}

.panel-subtitle {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
  font-size: 10.5px;
  color: var(--text-tertiary);
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
  padding: 0 14px;
  padding-bottom: 8px;
  flex-shrink: 0;
  overflow-x: auto;
}

.filter-tab {
  height: 26px;
  padding: 0 10px;
  border-radius: 5px;
  font-size: 11.5px;
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
  font-size: 10px;
  background: var(--bg-active);
  padding: 1px 5px;
  border-radius: 4px;
  color: var(--text-tertiary);
}

.filter-tab.active .filter-count {
  background: var(--accent-soft);
  color: var(--accent);
}

.panel-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  position: relative;
}

.stats-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6px;
  padding: 0 14px 10px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.stat-pill {
  min-width: 0;
  border: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
  border-radius: var(--radius-sm);
  padding: 7px 8px;
}

.stat-value {
  display: block;
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
  color: var(--accent);
}

.stat-label {
  display: block;
  margin-top: 1px;
  font-size: 10px;
  color: var(--text-tertiary);
}

.stat-pill.warning .stat-value {
  color: var(--warning);
}

.stat-pill.sensitive .stat-value {
  color: var(--sensitive);
}

/* Batch bar */
.batch-bar {
  padding: 8px 14px;
  background: var(--accent-soft);
  border-bottom: 1px solid rgba(99, 102, 241, 0.15);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.batch-info {
  font-size: 11.5px;
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
  font-size: 11px;
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
  border-color: rgba(248, 113, 113, 0.2);
}

.batch-btn.danger-btn:hover {
  background: rgba(248, 113, 113, 0.2);
}
</style>
