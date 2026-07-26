<template>
  <div class="capture-status" :class="{ compact }">
    <span class="live-dot" :class="{ paused: clipboardStore.pauseCapture }"></span>
    <span class="status-text">{{ clipboardStore.pauseCapture ? $t('capture.paused') : $t('capture.capturing') }}</span>
    <button
      class="capture-toggle"
      :class="{ paused: clipboardStore.pauseCapture }"
      @click="clipboardStore.togglePauseCapture()"
    >
      {{ clipboardStore.pauseCapture ? $t('capture.resume') : $t('capture.pause') }}
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
  gap: var(--space-2);
}

.capture-status.compact {
  gap: var(--space-2);
}

.capture-status.compact .status-text {
  font-size: var(--text-xs);
}

.live-dot {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-pill);
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
  flex-shrink: 0;
}

.live-dot.paused {
  background: var(--warning);
  box-shadow: 0 0 0 3px var(--warning-soft);
}

.status-text {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.capture-toggle {
  height: 24px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: 600;
  background: var(--success-soft);
  color: var(--success);
  border: 1px solid color-mix(in srgb, var(--success) 18%, transparent);
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.capture-toggle.paused {
  background: var(--warning-soft);
  color: var(--warning);
  border-color: color-mix(in srgb, var(--warning) 22%, transparent);
}
</style>
