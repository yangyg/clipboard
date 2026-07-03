<template>
  <div class="action-bar">
    <template v-if="record && !record.is_trashed">
      <button class="btn btn-primary" @click="paste">⏎ 粘贴</button>
      <button class="btn btn-secondary" @click="pastePlain">Aa 纯文本</button>
      <button
        class="btn btn-secondary"
        :class="{ 'btn-active': record.is_favorite }"
        @click="toggleFav"
      >
        {{ record.is_favorite ? '★ 已收藏' : '☆ 收藏' }}
      </button>
      <button class="btn btn-secondary" @click="pin">
        {{ record.is_pinned ? '📌 已置顶' : '📌 置顶' }}
      </button>
      <button class="btn btn-secondary" @click="del">🗑</button>
    </template>
    <template v-else-if="record && record.is_trashed">
      <button class="btn btn-primary" @click="restore">↩ 恢复</button>
      <button class="btn btn-danger" @click="permanentDel">🗑 永久删除</button>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useClipboardStore } from "../stores/clipboard";

const clipboardStore = useClipboardStore();
const record = computed(() => clipboardStore.selectedRecord);

function paste() {
  if (record.value) clipboardStore.pasteRecord(record.value.id);
}

function pastePlain() {
  if (record.value) clipboardStore.pasteRecord(record.value.id, "plain");
}

function toggleFav() {
  if (record.value) clipboardStore.toggleFavorite(record.value.id);
}

function pin() {
  if (record.value) clipboardStore.togglePin(record.value.id);
}

function del() {
  if (record.value && confirm("确定要将这条记录移到回收站吗？")) {
    clipboardStore.deleteRecord(record.value.id);
  }
}

function restore() {
  if (record.value) clipboardStore.restoreRecord(record.value.id);
}

function permanentDel() {
  if (record.value && confirm("确定要永久删除这条记录吗？此操作不可恢复。")) {
    clipboardStore.permanentlyDeleteRecord(record.value.id);
  }
}
</script>

<style scoped>
.action-bar {
  padding: 10px 14px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  background: var(--bg-surface);
  transition: background var(--transition-smooth), border-color var(--transition-smooth);
}

.btn-active {
  color: var(--warning) !important;
  background: var(--warning-soft) !important;
  border-color: rgba(251, 191, 36, 0.2) !important;
}
</style>
