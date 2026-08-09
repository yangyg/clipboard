<!-- Preview area of the record screen: side-by-side column (wide host) or
     overlay drawer (tight host), plus the drag/keyboard resizer between the
     list and preview columns. Width state stays in the parent so the list
     column can consume it too. -->
<template>
  <!-- Resizer between list and preview (side-by-side only) -->
  <div
    v-if="showResizer"
    class="resizer"
    :class="{ active: dragging }"
    role="separator"
    aria-orientation="vertical"
    :aria-valuenow="colWidth"
    :aria-valuemin="colMin"
    :aria-valuemax="colMax"
    tabindex="0"
    :aria-label="$t('record.resizeList')"
    @pointerdown="emit('resize-start', $event)"
    @keydown="emit('resize-key', $event)"
  />

  <div v-if="visible && drawer" class="preview-drawer-backdrop" @click="emit('close')" />
  <div
    v-if="showHost"
    class="preview-host"
    :class="{ 'preview-host--drawer': drawer }"
  >
    <PreviewPane :drawer="drawer" />
  </div>
</template>

<script setup lang="ts">
import PreviewPane from "./PreviewPane.vue";

defineProps<{
  /** A record is selected (and not in batch mode). */
  visible: boolean;
  /** Host too tight for side-by-side → overlay drawer instead. */
  drawer: boolean;
  /** Render the host container (persistent column in wide layouts). */
  showHost: boolean;
  /** Show the drag separator (side-by-side only). */
  showResizer: boolean;
  colWidth: number;
  colMin: number;
  colMax: number;
  dragging: boolean;
}>();

const emit = defineEmits<{
  close: [];
  "resize-start": [event: PointerEvent];
  "resize-key": [event: KeyboardEvent];
}>();
</script>

<style scoped>
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
  /* Overlay the list column's right edge instead of reserving flex space,
     keeping the list/preview layout fully compact. z-index keeps it above
     the column's positioned rows so hover/drag pointer events still land. */
  margin-left: -4px;
  position: relative;
  z-index: 10;
  cursor: col-resize;
  background: transparent;
  flex-shrink: 0;
  transition: background var(--transition-fast);
  touch-action: none;
}

.resizer:hover,
.resizer.active {
  background: var(--accent);
}

.resizer:focus-visible {
  background: var(--accent);
  outline: none;
}
</style>
