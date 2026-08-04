import { describe, it, expect } from "vitest";
import {
  SOURCE_AVATAR_PALETTE,
  buildSourceOverrides,
  resolveSourceBadge,
  resolveSourceLabel,
  sourceAvatarColor,
  sourceInitial,
  sourceShortName,
  type TranslateFn,
} from "./sourceBadge";

const translations: Record<string, string> = {
  "record.systemClipboard": "系统剪贴板",
  "sourceNames.notepad": "记事本",
  "sourceNames.paint": "画图",
};
const t: TranslateFn = (key: string) => translations[key] ?? key;

describe("sourceBadge", () => {
  it("maps empty source to 系统剪贴板 / 剪 / gray", () => {
    expect(sourceShortName("")).toBe("系统剪贴板");
    expect(sourceShortName("   ")).toBe("系统剪贴板");
    expect(sourceInitial("系统剪贴板", "")).toBe("剪");
    expect(sourceAvatarColor("")).toBe("var(--text-tertiary)");
    const badge = resolveSourceBadge("", undefined, t);
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

  it("resolves system apps through the i18n key", () => {
    expect(resolveSourceLabel("notepad.exe", undefined, t)).toBe("记事本");
    expect(resolveSourceLabel("C:\\Windows\\System32\\mspaint.exe", undefined, t)).toBe(
      "画图",
    );
  });

  it("resolves known brands to fixed names", () => {
    expect(resolveSourceLabel("wechat.exe", undefined, t)).toBe("微信");
    expect(resolveSourceLabel("QQ.exe", undefined, t)).toBe("QQ");
    expect(resolveSourceLabel("chrome.exe", undefined, t)).toBe("Chrome");
  });

  it("brand names override the FileDescription source_name", () => {
    expect(resolveSourceLabel("wechat.exe", "WeChat", t)).toBe("微信");
  });

  it("falls back to source_name for unknown apps", () => {
    expect(resolveSourceLabel("someapp.exe", "某软件", t)).toBe("某软件");
  });

  it("falls back to the exe short name when nothing else applies", () => {
    expect(resolveSourceLabel("unknownapp.exe", undefined, t)).toBe("unknownapp");
    expect(resolveSourceLabel("", undefined, t)).toBe("系统剪贴板");
  });

  it("user overrides win over brands and FileDescription", () => {
    const overrides = { "wechat.exe": "WeChat" };
    expect(resolveSourceLabel("wechat.exe", "微信", t, overrides)).toBe("WeChat");
    expect(resolveSourceLabel("myapp.exe", "某软件", t, overrides)).toBe("某软件");
  });

  it("builds a normalized override map from the settings list", () => {
    const map = buildSourceOverrides([
      { exe_name: "MyApp.exe", display_name: " 我的应用 " },
      { exe_name: "  ", display_name: "no exe" },
      { exe_name: "empty.exe", display_name: "   " },
    ]);
    expect(map).toEqual({ "myapp.exe": "我的应用" });
    expect(buildSourceOverrides(undefined)).toEqual({});
    expect(buildSourceOverrides([])).toEqual({});
  });
});
