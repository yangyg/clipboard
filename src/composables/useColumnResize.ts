import { ref, onUnmounted, type Ref } from "vue";

export interface ColumnResizeOptions {
  /** localStorage key for persistence. */
  storageKey: string;
  /** Initial width (px) when no stored value exists. */
  defaultWidth: number;
  /** Minimum allowed width (px). */
  min: number;
  /** Maximum allowed width (px). */
  max: number;
  /**
   * When true, dragging is disabled and the host should not apply
   * the inline width (letting CSS media-query rules take over).
   */
  disabled?: Ref<boolean>;
}

/**
 * Drag-to-resize a column via pointer events + requestAnimationFrame.
 *
 * Returns a reactive `width` and a `startResize` handler to bind on the
 * resizer element's `@pointerdown`. Width is persisted to localStorage
 * on drag end and restored on init.
 */
export function useColumnResize(options: ColumnResizeOptions) {
  const { storageKey, defaultWidth, min, max, disabled } = options;

  function readStored(): number {
    try {
      const v = localStorage.getItem(storageKey);
      if (v != null) {
        const n = parseInt(v, 10);
        if (!Number.isNaN(n)) return Math.min(max, Math.max(min, n));
      }
    } catch {
      /* ignore */
    }
    return defaultWidth;
  }

  const width = ref(readStored());
  const isDragging = ref(false);
  /** True when no stored value was found (first run). */
  const isDefault = ref(!hasStored());

  function hasStored(): boolean {
    try {
      return localStorage.getItem(storageKey) != null;
    } catch {
      return false;
    }
  }

  /** Programmatically set width (e.g. capture from DOM). Clears isDefault. */
  function setWidth(w: number) {
    width.value = Math.min(max, Math.max(min, w));
    isDefault.value = false;
  }

  let raf = 0;
  let startX = 0;
  let startW = 0;

  function onPointerMove(e: PointerEvent) {
    if (raf) return; // throttle to one rAF per frame
    raf = requestAnimationFrame(() => {
      raf = 0;
      const delta = e.clientX - startX;
      width.value = Math.min(max, Math.max(min, startW + delta));
    });
  }

  function onPointerUp() {
    cleanupDrag();
    try {
      localStorage.setItem(storageKey, String(width.value));
    } catch {
      /* ignore */
    }
  }

  function cleanupDrag() {
    if (raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
    isDragging.value = false;
    document.removeEventListener("pointermove", onPointerMove);
    document.removeEventListener("pointerup", onPointerUp);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  function startResize(e: PointerEvent) {
    if (disabled?.value) return;
    e.preventDefault();
    isDragging.value = true;
    startX = e.clientX;
    startW = width.value;
    document.addEventListener("pointermove", onPointerMove);
    document.addEventListener("pointerup", onPointerUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  onUnmounted(() => {
    cleanupDrag();
  });

  return { width, isDragging, isDefault, startResize, setWidth };
}
