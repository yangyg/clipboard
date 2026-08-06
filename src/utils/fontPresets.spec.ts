import { describe, it, expect } from "vitest";
import {
  FONT_PRESETS,
  SYSTEM_FONT_FALLBACK,
  isSystemFontValue,
  resolveFontStack,
  systemFontName,
} from "@/utils/fontPresets";

describe("fontPresets", () => {
  it("exposes six presets with a default first", () => {
    expect(FONT_PRESETS.map((p) => p.key)).toEqual([
      "default",
      "yahei",
      "simhei",
      "simsun",
      "kaiti",
      "segoe",
    ]);
    expect(FONT_PRESETS[0].key).toBe("default");
  });

  it("resolves preset keys to their stacks", () => {
    const defaultStack = resolveFontStack("default");
    expect(defaultStack).toContain("Noto Sans SC");
    expect(resolveFontStack("yahei")).toContain("Microsoft YaHei");
  });

  it("treats system: values as system fonts", () => {
    expect(isSystemFontValue("system:SimSun")).toBe(true);
    expect(isSystemFontValue("simsun")).toBe(false);
    expect(systemFontName("system:KaiTi")).toBe("KaiTi");
    expect(systemFontName("kaiti")).toBe("");
  });

  it("resolves system: values with a CJK-safe fallback stack", () => {
    const stack = resolveFontStack("system:KaiTi");
    expect(stack).toContain('"KaiTi"');
    expect(stack).toContain(SYSTEM_FONT_FALLBACK);
  });

  it("falls back to the default stack for unknown values", () => {
    expect(resolveFontStack("no-such-preset")).toBe(resolveFontStack("default"));
    expect(resolveFontStack("system:")).toBe(resolveFontStack("default"));
  });

  it("quotes a system family name containing spaces", () => {
    const stack = resolveFontStack("system:Microsoft YaHei UI");
    expect(stack).toContain('"Microsoft YaHei UI"');
  });
});
