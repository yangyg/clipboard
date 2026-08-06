import { describe, it, expect } from "vitest";
import {
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
});
