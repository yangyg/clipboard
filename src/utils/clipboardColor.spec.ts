import { describe, it, expect } from "vitest";
import { expandHexColor, parseClipboardColor } from "./clipboardColor";

describe("clipboardColor", () => {
  it("parses hex 3/6/8", () => {
    expect(parseClipboardColor("#07d")).toBe("#0077dd");
    expect(parseClipboardColor("#0078D4")).toBe("#0078d4");
    expect(parseClipboardColor("  #0078d4ff ")).toBe("#0078d4ff");
  });

  it("parses rgb/hsl forms", () => {
    expect(parseClipboardColor("rgb(0, 120, 212)")).toBe("rgb(0, 120, 212)");
    expect(parseClipboardColor("rgba(0, 120, 212, 0.5)")).toBe("rgba(0, 120, 212, 0.5)");
    expect(parseClipboardColor("hsl(210, 100%, 42%)")).toBe("hsl(210, 100%, 42%)");
    expect(parseClipboardColor("rgb(0 120 212)")).toBe("rgb(0 120 212)");
  });

  it("rejects non-standalone or long text", () => {
    expect(parseClipboardColor("color: #0078d4")).toBeNull();
    expect(parseClipboardColor("#0078d4 is nice")).toBeNull();
    expect(parseClipboardColor("red")).toBeNull();
    expect(parseClipboardColor("")).toBeNull();
    expect(parseClipboardColor("a".repeat(65))).toBeNull();
  });

  it("expandHexColor pads short forms", () => {
    expect(expandHexColor("#abc")).toBe("#aabbcc");
    expect(expandHexColor("#abcd")).toBe("#aabbccdd");
  });
});
