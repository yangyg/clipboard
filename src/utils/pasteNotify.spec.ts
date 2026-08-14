import { describe, it, expect, vi } from "vitest";
import { toastPasteOutcome } from "./pasteNotify";

describe("toastPasteOutcome", () => {
  it("tells the user to paste manually when Ctrl+V was skipped", () => {
    const toast = vi.fn();
    const t = (key: string) => key;
    toastPasteOutcome(false, "original", t, toast);
    expect(toast).toHaveBeenCalledWith("record.copiedToClipboard", "success");
  });

  it("uses the plain-text copy when keys were sent in plain mode", () => {
    const toast = vi.fn();
    const t = (key: string) => key;
    toastPasteOutcome(true, "plain", t, toast);
    expect(toast).toHaveBeenCalledWith("record.pastedPlain", "success");
  });
});
