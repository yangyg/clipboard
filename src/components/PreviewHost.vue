<!-- Preview area: persistent/flex column, on-demand fixed column, or
     overlay drawer — chosen by settings.preview_layout. Width state
     stays in the parent. -->
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
    :aria-label="fixedColumn ? $t('record.resizePreview') : $t('record.resizeList')"
    @pointerdown="emit('resize-start', $event)"
    @keydown="emit('resize-key', $event)"
  />

  <div v-if="visible && drawer" class="preview-drawer-backdrop" @click="emit('close')" />
  <div
    v-if="showHost"
    class="preview-host"
    :class="{ 'preview-host--drawer': drawer, 'preview-host--column': !drawer }"
    :style="columnStyle"
  >
    <PreviewPane />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import PreviewPane from "./PreviewPane.vue";

const props = defineProps<{
  /** Preview host is showing (column, empty column, or drawer). */
  visible: boolean;
  /** Overlay drawer instead of a side-by-side column. */
  drawer: boolean;
  /** Render the host container. */
  showHost: boolean;
  /** Show the drag separator (side-by-side only). */
  showResizer: boolean;
  /** When true, the preview column uses a stored width (on-demand). */
  fixedColumn: boolean;
  colWidth: number;
  colMin: number;
  colMax: number;
  dragging: boolean;
}>();

const columnStyle = computed(() => {
  if (props.drawer || !props.fixedColumn) return undefined;
  return {
    width: `${props.colWidth}px`,
    minWidth: `${props.colWidth}px`,
    flex: "none",
  };
});

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

.preview-host--column {
  animation: preview-column-in var(--transition-smooth);
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

:global(body.anim-disabled) .preview-host--column,
:global(body.anim-disabled) .preview-host--drawer,
:global(body.anim-disabled) .preview-drawer-backdrop {
  animation: none;
}

@media (prefers-reduced-motion: reduce) {
  .preview-host--column,
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

@keyframes preview-column-in {
  from {
    transform: translateX(12px);
    opacity: 0.6;
  }
  to {
    transform: none;
    opacity: 1;
  }
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
