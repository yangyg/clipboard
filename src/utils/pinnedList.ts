/** localStorage key for the middle-column pinned-section fold (session-independent). */
export const PINNED_COLLAPSED_KEY = "clipvault-pinned-collapsed";

export function readPinnedCollapsed(): boolean {
  try {
    return localStorage.getItem(PINNED_COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function persistPinnedCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(PINNED_COLLAPSED_KEY, collapsed ? "1" : "0");
  } catch {
    /* quota / private mode */
  }
}

export const RECORD_OPTION_PREFIX = "record-option-";
export const DOCK_OPTION_PREFIX = "dock-record-option-";

/** Focus the visible listbox option — prefer the dock copy when that layer is shown. */
export function focusRecordOption(id: number): void {
  const dock = document.getElementById(`${DOCK_OPTION_PREFIX}${id}`);
  if (dock instanceof HTMLElement && dock.offsetParent !== null) {
    dock.focus({ preventScroll: true });
    return;
  }
  document.getElementById(`${RECORD_OPTION_PREFIX}${id}`)?.focus({ preventScroll: true });
}

/**
 * Virtual-list slots below the in-flow pinned block: unpinned records only.
 * While the pinned group is expanded and both groups exist, a divider sits
 * before the first unpinned row.
 */
export type PinnedListSlot =
  | { type: "divider" }
  | { type: "record"; id: number };

export function pinnedListSlots<T extends { id: number; is_pinned: boolean }>(
  records: readonly T[],
  pinnedCollapsed: boolean,
): PinnedListSlot[] {
  let hasPinned = false;
  let hasUnpinned = false;
  for (const r of records) {
    if (r.is_pinned) hasPinned = true;
    else hasUnpinned = true;
    if (hasPinned && hasUnpinned) break;
  }
  const slots: PinnedListSlot[] = [];
  let dividerInserted = false;
  for (const r of records) {
    if (hasPinned && hasUnpinned && !r.is_pinned && !dividerInserted) {
      if (!pinnedCollapsed) slots.push({ type: "divider" });
      dividerInserted = true;
    }
    if (r.is_pinned) continue;
    slots.push({ type: "record", id: r.id });
  }
  return slots;
}

/** Arrow-key / batch-visible rows: hide pinned records while the section is folded. */
export function visibleListRecords<T extends { is_pinned: boolean }>(
  records: readonly T[],
  pinnedCollapsed: boolean,
): T[] {
  if (!pinnedCollapsed) return records as T[];
  return records.filter((r) => !r.is_pinned);
}
