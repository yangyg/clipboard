import { describe, it, expect } from "vitest";
import {
  nearestPaletteColor,
  normalizeColorKey,
  resolveTagPalette,
  TAG_PALETTE_HEX,
  TAG_PALETTE_SIZE,
  uniqueColors,
} from "./themeColors";

describe("themeColors palette", () => {
  it("uniqueColors drops case/whitespace duplicates", () => {
    expect(uniqueColors(["#6366F1", " #6366f1 ", "#818cf8", "#6366f1"])).toEqual([
      "#6366F1",
      "#818cf8",
    ]);
  });

  it("normalizeColorKey lowercases hex", () => {
    expect(normalizeColorKey("  #AbCdEf  ")).toBe("#abcdef");
  });

  it("has 12 unique hue-wheel swatches", () => {
    expect(TAG_PALETTE_HEX).toHaveLength(TAG_PALETTE_SIZE);
    expect(new Set(TAG_PALETTE_HEX.map(normalizeColorKey)).size).toBe(TAG_PALETTE_SIZE);
  });

  it("resolveTagPalette returns exactly 12 unique swatches", () => {
    const palette = resolveTagPalette();
    expect(palette).toHaveLength(TAG_PALETTE_SIZE);
    expect(new Set(palette.map(normalizeColorKey)).size).toBe(TAG_PALETTE_SIZE);
  });

  it("keeps existing tag colors and still caps at 12", () => {
    const custom = "#7c5cfc";
    const palette = resolveTagPalette([custom, "#6366F1"]);
    expect(palette).toHaveLength(TAG_PALETTE_SIZE);
    expect(palette.map(normalizeColorKey)).toContain(normalizeColorKey(custom));
    expect(new Set(palette.map(normalizeColorKey)).size).toBe(TAG_PALETTE_SIZE);
  });

  it("nearestPaletteColor returns exact match unchanged", () => {
    expect(nearestPaletteColor("#3B82F6")).toBe("#3b82f6");
    expect(normalizeColorKey(nearestPaletteColor("  #22C55E  "))).toBe("#22c55e");
  });

  it("nearestPaletteColor snaps off-palette colors onto the wheel", () => {
    const palette = new Set(TAG_PALETTE_HEX.map(normalizeColorKey));
    for (const legacy of ["#0078d4", "#60cdff", "#34d399", "#fbbf24", "#a78bfa"]) {
      const snapped = normalizeColorKey(nearestPaletteColor(legacy));
      expect(palette.has(snapped)).toBe(true);
    }
  });

  it("nearestPaletteColor falls back on invalid input", () => {
    expect(nearestPaletteColor("not-a-color")).toBe(TAG_PALETTE_HEX[0]);
    expect(nearestPaletteColor("")).toBe(TAG_PALETTE_HEX[0]);
  });
});
