import { describe, expect, it } from "vitest";
import { isThemeKey, THEME_DEFINITIONS } from "./themeRegistry";

describe("theme registry", () => {
  it("contains the editorial and sticker variants", () => {
    expect(THEME_DEFINITIONS.map((theme) => theme.key)).toEqual(
      expect.arrayContaining([
        "editorial",
        "editorial-light",
        "sticker",
        "sticker-light",
      ]),
    );
  });

  it("accepts registered themes and rejects unknown values", () => {
    expect(isThemeKey("editorial")).toBe(true);
    expect(isThemeKey("sticker-light")).toBe(true);
    expect(isThemeKey("not-a-theme")).toBe(false);
  });
});
