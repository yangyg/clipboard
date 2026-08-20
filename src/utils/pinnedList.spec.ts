import { describe, expect, it, vi } from "vitest";
import {
  persistPinnedCollapsed,
  PINNED_COLLAPSED_KEY,
  pinnedListSlots,
  readPinnedCollapsed,
  visibleListRecords,
  focusRecordOption,
  DOCK_OPTION_PREFIX,
  RECORD_OPTION_PREFIX,
} from "./pinnedList";

const rows = [
  { id: 1, is_pinned: true },
  { id: 2, is_pinned: true },
  { id: 3, is_pinned: false },
  { id: 4, is_pinned: false },
];

describe("pinnedListSlots", () => {
  it("emits a divider + unpinned rows when expanded (pinned rows are in-flow chrome)", () => {
    expect(pinnedListSlots(rows, false)).toEqual([
      { type: "divider" },
      { type: "record", id: 3 },
      { type: "record", id: 4 },
    ]);
  });

  it("emits only unpinned rows when collapsed", () => {
    expect(pinnedListSlots(rows, true)).toEqual([
      { type: "record", id: 3 },
      { type: "record", id: 4 },
    ]);
  });

  it("emits nothing when every row is pinned", () => {
    expect(
      pinnedListSlots(
        [
          { id: 1, is_pinned: true },
          { id: 2, is_pinned: true },
        ],
        false,
      ),
    ).toEqual([]);
  });

  it("omits the label and divider when nothing is pinned", () => {
    expect(
      pinnedListSlots(
        [
          { id: 3, is_pinned: false },
          { id: 4, is_pinned: false },
        ],
        true,
      ),
    ).toEqual([
      { type: "record", id: 3 },
      { type: "record", id: 4 },
    ]);
  });
});

describe("visibleListRecords", () => {
  it("returns the same list while expanded", () => {
    expect(visibleListRecords(rows, false)).toBe(rows);
  });

  it("drops pinned rows while collapsed", () => {
    expect(visibleListRecords(rows, true).map((r) => r.id)).toEqual([3, 4]);
  });
});

describe("pinnedCollapsed persistence", () => {
  it("round-trips through localStorage", () => {
    persistPinnedCollapsed(true);
    expect(localStorage.getItem(PINNED_COLLAPSED_KEY)).toBe("1");
    expect(readPinnedCollapsed()).toBe(true);
    persistPinnedCollapsed(false);
    expect(localStorage.getItem(PINNED_COLLAPSED_KEY)).toBe("0");
    expect(readPinnedCollapsed()).toBe(false);
  });
});

describe("focusRecordOption", () => {
  it("prefers a visible dock copy over the in-flow option", () => {
    const flow = document.createElement("button");
    flow.id = `${RECORD_OPTION_PREFIX}9`;
    const dock = document.createElement("button");
    dock.id = `${DOCK_OPTION_PREFIX}9`;
    document.body.append(flow, dock);
    Object.defineProperty(dock, "offsetParent", { value: document.body, configurable: true });
    const focus = vi.spyOn(dock, "focus");
    focusRecordOption(9);
    expect(focus).toHaveBeenCalled();
    flow.remove();
    dock.remove();
  });
});
