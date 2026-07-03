<template>
  <div class="capture-status" :class="{ compact }">
    <span class="live-dot" :class="{ paused: clipboardStore.pauseCapture }"></span>
    <span class="status-text">{{ clipboardStore.pauseCapture ? '捕获已暂停' : '正在捕获剪贴板' }}</span>
    <button
      class="capture-toggle"
      :class="{ paused: clipboardStore.pauseCapture }"
      @click="clipboardStore.togglePauseCapture()"
    >
      {{ clipboardStore.pauseCapture ? '恢复' : '暂停' }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { useClipboardStore } from "../stores/clipboard";

defineProps<{
  compact?: boolean;
}>();

const clipboardStore = useClipboardStore();
</script>

<style scoped>
.capture-status {
  display: flex;
  align-items: center;
  gap: 7px;
}

.capture-status.compact {
  gap: 6px;
}

.capture-status.compact .status-text {
  font-size: 10.5px;
}

.live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
  flex-shrink: 0;
}

.live-dot.paused {
  background: var(--warning);
  box-shadow: 0 0 0 3px var(--warning-soft);
}

.status-text {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.capture-toggle {
  height: 24px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-weight: 600;
  background: var(--success-soft);
  color: var(--success);
  border: 1px solid rgba(52, 211, 153, 0.18);
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.capture-toggle.paused {
  background: var(--warning-soft);
  color: var(--warning);
  border-color: rgba(251, 191, 36, 0.22);
}
</style>
