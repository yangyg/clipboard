import { describe, expect, it } from "vitest";
import {
  persistPinnedCollapsed,
  PINNED_COLLAPSED_KEY,
  pinnedListSlots,
  readPinnedCollapsed,
  visibleListRecords,
} from "./pinnedList";

const rows = [
  { id: 1, is_pinned: true },
  { id: 2, is_pinned: true },
  { id: 3, is_pinned: false },
  { id: 4, is_pinned: false },
];

describe("pinnedListSlots", () => {
  it("emits label + pinned rows + divider + unpinned rows when expanded", () => {
    expect(pinnedListSlots(rows, false)).toEqual([
      { type: "label" },
      { type: "record", id: 1 },
      { type: "record", id: 2 },
      { type: "divider" },
      { type: "record", id: 3 },
      { type: "record", id: 4 },
    ]);
  });

  it("keeps the label and divider but drops pinned rows when collapsed", () => {
    expect(pinnedListSlots(rows, true)).toEqual([
      { type: "label" },
      { type: "divider" },
      { type: "record", id: 3 },
      { type: "record", id: 4 },
    ]);
  });

  it("shows only the label when every row is pinned and collapsed", () => {
    expect(
      pinnedListSlots(
        [
          { id: 1, is_pinned: true },
          { id: 2, is_pinned: true },
        ],
        true,
      ),
    ).toEqual([{ type: "label" }]);
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
