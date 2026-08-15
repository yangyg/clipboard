import { describe, expect, it } from "vitest";
import {
  LIST_MIN,
  PREVIEW_DEFAULT,
  PREVIEW_DRAWER_BREAKPOINT,
  PREVIEW_MAX,
  PREVIEW_MIN,
  clampPreviewWidth,
  normalizePreviewLayoutPref,
  resolvePreviewChrome,
} from "./previewLayout";

describe("normalizePreviewLayoutPref", () => {
  it("keeps known values and falls back to on_demand", () => {
    expect(normalizePreviewLayoutPref("columns")).toBe("columns");
    expect(normalizePreviewLayoutPref("drawer")).toBe("drawer");
    expect(normalizePreviewLayoutPref("on_demand")).toBe("on_demand");
    expect(normalizePreviewLayoutPref("nope")).toBe("on_demand");
    expect(normalizePreviewLayoutPref(undefined)).toBe("on_demand");
  });
});

describe("resolvePreviewChrome", () => {
  it("hides preview in batch mode for every preference", () => {
    expect(resolvePreviewChrome("columns", true, true, 900)).toEqual({ kind: "hidden" });
    expect(resolvePreviewChrome("on_demand", true, true, 900)).toEqual({ kind: "hidden" });
    expect(resolvePreviewChrome("drawer", true, true, 900)).toEqual({ kind: "hidden" });
  });

  describe("columns", () => {
    it("keeps a flex preview column even with no selection", () => {
      expect(resolvePreviewChrome("columns", false, false, 900)).toEqual({
        kind: "column",
        sizing: "flex",
      });
    });

    it("falls back to a drawer when the host is too narrow and a record is selected", () => {
      expect(resolvePreviewChrome("columns", true, false, PREVIEW_DRAWER_BREAKPOINT - 1)).toEqual({
        kind: "drawer",
      });
    });

    it("hides the empty column when the host is too narrow", () => {
      expect(resolvePreviewChrome("columns", false, false, PREVIEW_DRAWER_BREAKPOINT - 1)).toEqual({
        kind: "hidden",
      });
    });
  });

  describe("on_demand", () => {
    it("hides preview when nothing is selected", () => {
      expect(resolvePreviewChrome("on_demand", false, false, 900)).toEqual({ kind: "hidden" });
    });

    it("uses a fixed-width column when selected and wide enough", () => {
      expect(resolvePreviewChrome("on_demand", true, false, PREVIEW_DRAWER_BREAKPOINT)).toEqual({
        kind: "column",
        sizing: "fixed",
      });
    });

    it("uses a drawer when the host is tighter than the breakpoint", () => {
      expect(resolvePreviewChrome("on_demand", true, false, PREVIEW_DRAWER_BREAKPOINT - 1)).toEqual({
        kind: "drawer",
      });
    });

    it("does not flash the drawer before the host is measured", () => {
      expect(resolvePreviewChrome("on_demand", true, false, 0)).toEqual({
        kind: "column",
        sizing: "fixed",
      });
    });
  });

  describe("drawer", () => {
    it("shows a drawer only when a record is selected", () => {
      expect(resolvePreviewChrome("drawer", true, false, 900)).toEqual({ kind: "drawer" });
      expect(resolvePreviewChrome("drawer", false, false, 900)).toEqual({ kind: "hidden" });
    });
  });
});

describe("clampPreviewWidth", () => {
  it("clamps to the preview min/max when the host is unmeasured", () => {
    expect(clampPreviewWidth(PREVIEW_DEFAULT, 0)).toBe(PREVIEW_DEFAULT);
    expect(clampPreviewWidth(100, 0)).toBe(PREVIEW_MIN);
    expect(clampPreviewWidth(800, 0)).toBe(PREVIEW_MAX);
  });

  it("keeps at least LIST_MIN for the list column", () => {
    expect(clampPreviewWidth(520, LIST_MIN + 300)).toBe(300);
  });

  it("does not shrink below PREVIEW_MIN even on a tight host", () => {
    expect(clampPreviewWidth(360, LIST_MIN + 100)).toBe(PREVIEW_MIN);
  });
});
