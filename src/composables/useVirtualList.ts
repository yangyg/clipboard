import {
  computed,
  nextTick,
  onUnmounted,
  ref,
  shallowRef,
  watch,
  type Ref,
} from "vue";
import { useClipboardStore } from "../stores/clipboard";
import { useSettingsStore } from "../stores/settings";
import { recordThumbSrc } from "../utils/mediaUrl";
import type { ClipboardRecord } from "../types";

export type ListLayout = "list" | "grid";

/** Layout-only row (no record payload — avoids rebuild on content/copy_count churn). */
export interface FlatItem {
  key: string;
  type: "label" | "divider" | "record";
  id?: number;
  height: number;
  offset: number;
}

export interface WindowItem extends FlatItem {
  record?: ClipboardRecord;
  thumb?: string | null;
}

/** A virtual row in grid layout: 1..N record cards, or a solo label/divider. */
interface GridRow {
  items: FlatItem[];
  height: number; // row content height (excludes gap)
  offset: number; // cumulative top offset including gaps
}

/** Row estimates scaled with UI font size (settings.font_size → --ui-font-scale). */
const BASE_ROW_HEIGHT = 68;
const BASE_LABEL_HEIGHT = 28;
const BASE_DIVIDER_HEIGHT = 17;
const OVERSCAN = 6;

// === Grid virtualization constants (base @ font_size=16; CSS scales via --ui-font-scale) ===
export const GRID_GAP = 8; // px, matches CSS .view-grid gap
const BASE_GRID_CARD_HEIGHT = 132;
const BASE_GRID_IMAGE_HEIGHT = 140;
/** Minimum card width used to derive the responsive grid column count. */
const GRID_MIN_CARD_WIDTH = 200;

/**
 * Virtual-scroll engine for the record list.
 *
 * Owns all row-layout math (flat items + grid rows), the scroll window, and the
 * responsive grid column count. The host component keeps rendering, item
 * actions, and layout preference; it consumes `displayItems` / pad heights and
 * forwards scroll events via `onListScroll`.
 *
 * Grid columns are derived from the live container width (ResizeObserver) and
 * applied by the host as an inline `grid-template-columns`, so CSS and the
 * row-grouping math can never drift apart.
 */
export function useVirtualList(
  listRef: Ref<HTMLElement | null>,
  listLayout: Ref<ListLayout>,
) {
  const clipboardStore = useClipboardStore();
  const settingsStore = useSettingsStore();

  const rowHeight = computed(() =>
    Math.round(BASE_ROW_HEIGHT * (settingsStore.settings.font_size / 16))
  );
  const labelHeight = computed(() =>
    Math.round(BASE_LABEL_HEIGHT * (settingsStore.settings.font_size / 16))
  );
  const dividerHeight = computed(() =>
    Math.round(BASE_DIVIDER_HEIGHT * (settingsStore.settings.font_size / 16))
  );
  const gridCardHeight = computed(() =>
    Math.round(BASE_GRID_CARD_HEIGHT * (settingsStore.settings.font_size / 16))
  );
  const gridImageHeight = computed(() =>
    Math.round(BASE_GRID_IMAGE_HEIGHT * (settingsStore.settings.font_size / 16))
  );

  const scrollTop = ref(0);
  const viewportHeight = ref(480);
  let scrollRaf = 0;

  /** Responsive grid column count (derived from container width). */
  const gridCols = ref(2);

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
    const maxRounds = 20;
    while (
      rounds < maxRounds &&
      clipboardStore.hasMore &&
      !clipboardStore.isLoadingMore &&
      el.scrollHeight <= el.clientHeight + 40
    ) {
      rounds += 1;
      const before = clipboardStore.records.length;
      await clipboardStore.loadMore();
      await nextTick();
      if (clipboardStore.records.length === before) break;
    }
  }

  // Explicit token from store after first-page load/search — not tied to isLoading churn.
  watch(
    () => clipboardStore.viewportFillToken,
    () => {
      void fillViewportIfNeeded();
    }
  );

  /** When a new row is prepended while scrolled, keep the viewport anchored. */
  watch(
    () => clipboardStore.lastIncomingId,
    async (id) => {
      if (id == null) return;
      const el = listRef.value;
      if (!el || el.scrollTop <= 4) return;
      const delta =
        listLayout.value === "grid"
          ? gridCardHeight.value + GRID_GAP
          : rowHeight.value;
      await nextTick();
      el.scrollTop += delta;
      scrollTop.value = el.scrollTop;
    }
  );

  function gridItemHeight(item: FlatItem): number {
    if (item.type === "label") return item.height;
    if (item.type === "divider") return item.height;
    // record: image cards are taller
    const records = clipboardStore.filteredRecords;
    const idx = recordIndexById.value.get(item.id!);
    const r = idx !== undefined ? records[idx] : undefined;
    return r && r.content_type === "image" ? gridImageHeight.value : gridCardHeight.value;
  }

  /** Group flatItems into grid rows (gridCols records per row; labels/dividers solo). */
  function buildGridRows(items: FlatItem[]): GridRow[] {
    const rows: GridRow[] = [];
    const cols = gridCols.value;
    let offset = 0;
    let i = 0;
    while (i < items.length) {
      const item = items[i];
      if (item.type !== "record") {
        // Label or divider: full-width solo row
        rows.push({ items: [item], height: item.height, offset });
        offset += item.height + GRID_GAP;
        i++;
      } else {
        // Pair up to `cols` record items into one row
        const rowItems: FlatItem[] = [item];
        let rowH = gridItemHeight(item);
        i++;
        while (rowItems.length < cols && i < items.length && items[i].type === "record") {
          rowItems.push(items[i]);
          rowH = Math.max(rowH, gridItemHeight(items[i]));
          i++;
        }
        rows.push({ items: rowItems, height: rowH, offset });
        offset += rowH + GRID_GAP;
      }
    }
    return rows;
  }

  const gridRows = shallowRef<GridRow[]>([]);

  // Also build on layout switch
  watch(listLayout, (v) => {
    if (v === "grid") {
      gridRows.value = buildGridRows(flatItems.value);
    }
  });

  /** M-2: Numeric layout signature — detects id order / pin flags / row height
   * changes without O(N) string concatenation. Uses FNV-style hash (32-bit).
   * Collision probability negligible for ≤1000 records (list soft cap = 120). */
  const layoutSig = computed(() => {
    const records = clipboardStore.filteredRecords;
    // Incorporate row heights (change on font-size setting).
    let h = rowHeight.value * 2654435761;
    h = (h ^ (labelHeight.value * 40503)) >>> 0;
    h = (h ^ (dividerHeight.value * 12347)) >>> 0;
    // Mix in record count + id/pin per record.
    h = (h ^ records.length) >>> 0;
    for (const r of records) {
      // id is unique; pin flag in high bit. Multiply-xor for order sensitivity.
      h = (h ^ ((r.id * 2654435761 + (r.is_pinned ? 0x9e3779b9 : 0)) >>> 0)) >>> 0;
      // Grid cards have type-dependent heights, so a type change must rebuild rows.
      const typeCode = r.content_type === "image" ? 1 : 0;
      h = (h ^ typeCode) >>> 0;
      h = ((h << 5) ^ (h >>> 27)) >>> 0; // rotate-mix
    }
    return h;
  });

  function buildFlatItems(): FlatItem[] {
    const records = clipboardStore.filteredRecords;
    const items: FlatItem[] = [];
    let offset = 0;
    const rh = rowHeight.value;
    const lh = labelHeight.value;
    const dh = dividerHeight.value;
    // Single pass to detect pin partition presence (avoids two O(N) .some() scans).
    let hasPinned = false;
    let hasUnpinned = false;
    for (const r of records) {
      if (r.is_pinned) hasPinned = true;
      else hasUnpinned = true;
      if (hasPinned && hasUnpinned) break;
    }
    if (hasPinned) {
      items.push({ key: "pinned-label", type: "label", height: lh, offset });
      offset += lh;
    }
    let dividerInserted = false;
    for (const r of records) {
      if (hasPinned && hasUnpinned && !r.is_pinned && !dividerInserted) {
        items.push({ key: "pin-divider", type: "divider", height: dh, offset });
        offset += dh;
        dividerInserted = true;
      }
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

  function buildRecordIndex(): Map<number, number> {
    const m = new Map<number, number>();
    clipboardStore.filteredRecords.forEach((r, i) => m.set(r.id, i));
    return m;
  }

  const flatItems = shallowRef<FlatItem[]>(buildFlatItems());
  /** id → index in filteredRecords; rebuilt with layout only (not on content churn). */
  const recordIndexById = shallowRef(buildRecordIndex());

  // Build grid rows eagerly when mounted already in grid layout: none of the
  // rebuild watchers below are immediate, so data present at setup would
  // otherwise render an empty grid until the next layout/data/width change.
  if (listLayout.value === "grid") {
    gridRows.value = buildGridRows(flatItems.value);
  }

  watch(layoutSig, () => {
    flatItems.value = buildFlatItems();
    recordIndexById.value = buildRecordIndex();
    // H-3: Keep grid rows in sync (must run AFTER flatItems + index update).
    if (listLayout.value === "grid") {
      gridRows.value = buildGridRows(flatItems.value);
    }
  });

  // Responsive: regroup grid rows whenever the column count changes.
  watch(gridCols, () => {
    if (listLayout.value === "grid") {
      gridRows.value = buildGridRows(flatItems.value);
    }
  });

  const contentHeight = computed(() => {
    const items = flatItems.value;
    if (items.length === 0) return 0;
    const last = items[items.length - 1];
    return last.offset + last.height;
  });

  /** First index where item.offset + item.height >= target (item not fully above target). */
  function lowerBoundPastTop(items: FlatItem[], target: number): number {
    let lo = 0;
    let hi = items.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (items[mid].offset + items[mid].height < target) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /** First index where item.offset >= target. */
  function lowerBoundByOffset(items: FlatItem[], target: number): number {
    let lo = 0;
    let hi = items.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (items[mid].offset < target) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  const virtualRange = computed(() => {
    const items = flatItems.value;
    const n = items.length;
    if (n === 0) return { start: 0, end: 0 };
    const top = scrollTop.value;
    const bottom = top + viewportHeight.value;
    let start = lowerBoundPastTop(items, top);
    let end = lowerBoundByOffset(items, bottom);
    start = Math.max(0, start - OVERSCAN);
    end = Math.min(n, end + OVERSCAN);
    return { start, end };
  });

  function resolveWindowItem(
    item: FlatItem,
    records: ClipboardRecord[],
    indexById: Map<number, number>
  ): WindowItem {
    if (item.type !== "record" || item.id == null) return item;
    const idx = indexById.get(item.id);
    const record = idx !== undefined ? records[idx] : undefined;
    if (!record) return item;
    return { ...item, record, thumb: recordThumbSrc(record) };
  }

  /** Grid virtual range: binary search on grid row offsets. */
  const gridVirtualRange = computed(() => {
    const rows = gridRows.value;
    const n = rows.length;
    if (n === 0) return { start: 0, end: 0 };
    const top = scrollTop.value;
    const bottom = top + viewportHeight.value;
    // First row whose bottom edge >= top
    let lo = 0, hi = n;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (rows[mid].offset + rows[mid].height < top) lo = mid + 1;
      else hi = mid;
    }
    let start = lo;
    // First row whose offset >= bottom
    lo = 0; hi = n;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (rows[mid].offset < bottom) lo = mid + 1;
      else hi = mid;
    }
    let end = lo;
    start = Math.max(0, start - OVERSCAN);
    end = Math.min(n, end + OVERSCAN);
    return { start, end };
  });

  const gridContentHeight = computed(() => {
    const rows = gridRows.value;
    if (rows.length === 0) return 0;
    const last = rows[rows.length - 1];
    return last.offset + last.height;
  });

  /** Grid: render items in visible rows; list: virtual window over flat items. */
  const displayItems = computed<WindowItem[]>(() => {
    const records = clipboardStore.filteredRecords;
    const indexById = recordIndexById.value;
    if (listLayout.value !== "grid") {
      const { start, end } = virtualRange.value;
      const slice = flatItems.value.slice(start, end);
      return slice.map((item) => resolveWindowItem(item, records, indexById));
    }
    // H-3: Grid virtualization — only resolve items in visible rows.
    const { start, end } = gridVirtualRange.value;
    const visibleRows = gridRows.value.slice(start, end);
    const result: WindowItem[] = [];
    for (const row of visibleRows) {
      for (const item of row.items) {
        result.push(resolveWindowItem(item, records, indexById));
      }
    }
    return result;
  });

  const virtualPadTop = computed(() => {
    if (listLayout.value === "grid") {
      const { start } = gridVirtualRange.value;
      const rows = gridRows.value;
      return start > 0 && rows.length > 0 ? rows[start].offset : 0;
    }
    const { start } = virtualRange.value;
    return start > 0 ? flatItems.value[start].offset : 0;
  });

  const virtualPadBottom = computed(() => {
    if (listLayout.value === "grid") {
      const { end } = gridVirtualRange.value;
      const rows = gridRows.value;
      if (end >= rows.length) return 0;
      return Math.max(0, gridContentHeight.value - rows[end].offset);
    }
    const { end } = virtualRange.value;
    const items = flatItems.value;
    if (end >= items.length) return 0;
    return Math.max(0, contentHeight.value - items[end].offset);
  });

  /** Derive the grid column count from the live content width. */
  function updateGridCols() {
    const el = listRef.value;
    if (!el) return;
    const cs = getComputedStyle(el);
    const contentWidth =
      el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    const cols = Math.max(
      1,
      Math.floor((contentWidth + GRID_GAP) / (GRID_MIN_CARD_WIDTH + GRID_GAP))
    );
    if (cols !== gridCols.value) {
      gridCols.value = cols;
    }
  }

  let resizeObserver: ResizeObserver | null = null;

  /** Create the ResizeObserver once (idempotent — listRef may appear after mount). */
  function ensureResizeObserver(el: HTMLElement) {
    if (!resizeObserver) {
      resizeObserver = new ResizeObserver(() => {
        const e = listRef.value;
        if (e) {
          viewportHeight.value = e.clientHeight;
          scrollTop.value = e.scrollTop;
        }
        updateGridCols();
      });
    }
    resizeObserver.disconnect();
    resizeObserver.observe(el);
  }

  // The list element mounts late when the initial render shows the loading /
  // empty state (v-if). A single watcher covers both mount orders: `immediate`
  // no-ops while the element is still null, then fires whenever the element
  // appears or is swapped after a v-if cycle (no onMounted duplicate).
  watch(
    listRef,
    (el) => {
      if (!el) return;
      viewportHeight.value = el.clientHeight;
      scrollTop.value = el.scrollTop;
      ensureResizeObserver(el);
      updateGridCols();
      void fillViewportIfNeeded();
    },
    { immediate: true }
  );

  onUnmounted(() => {
    if (scrollRaf) cancelAnimationFrame(scrollRaf);
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
  });

  return {
    displayItems,
    virtualPadTop,
    virtualPadBottom,
    flatItems,
    gridCols,
    scrollTop,
    onListScroll,
    fillViewportIfNeeded,
  };
}
