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

/**
 * Virtual-list slots for the pinned group: a label when any pin exists, a
 * divider between pin and unpinned blocks, then records. Collapsed mode keeps
 * the label (and divider if unpinned rows exist) but omits pinned records.
 */
export type PinnedListSlot =
  | { type: "label" }
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
  if (hasPinned) slots.push({ type: "label" });
  let dividerInserted = false;
  for (const r of records) {
    if (hasPinned && hasUnpinned && !r.is_pinned && !dividerInserted) {
      slots.push({ type: "divider" });
      dividerInserted = true;
    }
    if (pinnedCollapsed && r.is_pinned) continue;
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
