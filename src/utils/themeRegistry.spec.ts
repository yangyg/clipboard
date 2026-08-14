import { describe, expect, it } from "vitest";
import { applyTheme } from "./themeClass";
import { isThemeKey, THEME_DEFINITIONS } from "./themeRegistry";

describe("theme registry", () => {
  it("contains the editorial, sticker and flat variants", () => {
    expect(THEME_DEFINITIONS.map((theme) => theme.key)).toEqual(
      expect.arrayContaining([
        "editorial",
        "editorial-light",
        "sticker",
        "sticker-light",
        "flat",
        "flat-light",
        "pixel",
        "pixel-light",
      ]),
    );
  });

  it("accepts registered themes and rejects unknown values", () => {
    expect(isThemeKey("editorial")).toBe(true);
    expect(isThemeKey("sticker-light")).toBe(true);
    expect(isThemeKey("flat")).toBe(true);
    expect(isThemeKey("flat-light")).toBe(true);
    expect(isThemeKey("pixel")).toBe(true);
    expect(isThemeKey("pixel-light")).toBe(true);
    expect(isThemeKey("not-a-theme")).toBe(false);
  });

  it("maps legacy system and unknown keys to the dark default class", () => {
    applyTheme("light");
    expect(document.body.classList.contains("light-theme")).toBe(true);
    applyTheme("system");
    expect(document.body.classList.contains("light-theme")).toBe(false);
    applyTheme("not-a-theme");
    expect([...document.body.classList].some((c) => c.endsWith("-theme"))).toBe(false);
  });
});
