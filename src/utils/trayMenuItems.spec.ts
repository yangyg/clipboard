import { describe, it, expect } from "vitest";
import { buildTrayMenuItems } from "./trayMenuItems";

describe("buildTrayMenuItems", () => {
  it("shows 暂停捕获 when not paused", () => {
    const items = buildTrayMenuItems(false);
    expect(items.map((i) => i.id)).toEqual(["show", "pause", "settings", "quit"]);
    expect(items.find((i) => i.id === "pause")?.label).toBe("暂停捕获");
    expect(items.find((i) => i.id === "pause")?.icon).toBe("pause");
  });

  it("shows 恢复捕获 when paused", () => {
    const items = buildTrayMenuItems(true);
    expect(items.find((i) => i.id === "pause")?.label).toBe("恢复捕获");
    expect(items.find((i) => i.id === "pause")?.icon).toBe("play");
  });

  it("marks quit as danger and separators before settings/quit", () => {
    const items = buildTrayMenuItems(false);
    expect(items.find((i) => i.id === "settings")?.separatorBefore).toBe(true);
    expect(items.find((i) => i.id === "quit")?.separatorBefore).toBe(true);
    expect(items.find((i) => i.id === "quit")?.danger).toBe(true);
  });
});
