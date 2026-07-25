import { describe, it, expect } from "vitest";
import {
  SOURCE_AVATAR_PALETTE,
  resolveSourceBadge,
  sourceAvatarColor,
  sourceInitial,
  sourceShortName,
} from "./sourceBadge";

describe("sourceBadge", () => {
  it("maps empty source to 系统剪贴板 / 剪 / gray", () => {
    expect(sourceShortName("")).toBe("系统剪贴板");
    expect(sourceShortName("   ")).toBe("系统剪贴板");
    expect(sourceInitial("系统剪贴板", "")).toBe("剪");
    expect(sourceAvatarColor("")).toBe("var(--text-tertiary)");
    const badge = resolveSourceBadge("");
    expect(badge).toEqual({
      label: "系统剪贴板",
      initial: "剪",
      color: "var(--text-tertiary)",
    });
  });

  it("strips path and .exe for short name", () => {
    expect(sourceShortName("C:\\Program Files\\App\\msedge.exe")).toBe("msedge");
    expect(sourceShortName("/usr/bin/WorkBuddy")).toBe("WorkBuddy");
  });

  it("takes first latin/digit uppercase as initial", () => {
    expect(sourceInitial("msedge", "msedge")).toBe("M");
    expect(sourceInitial("WorkBuddy", "WorkBuddy")).toBe("W");
    expect(sourceInitial("应用App", "应用App")).toBe("A");
  });

  it("takes first character for non-latin short names", () => {
    expect(sourceInitial("微信", "微信")).toBe("微");
  });

  it("hashes the same source_app to the same palette color", () => {
    const a = sourceAvatarColor("msedge");
    const b = sourceAvatarColor("msedge");
    expect(a).toBe(b);
    expect(SOURCE_AVATAR_PALETTE).toContain(a);
  });
});
