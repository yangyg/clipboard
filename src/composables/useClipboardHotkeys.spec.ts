import { describe, it, expect } from "vitest";
import { resolveListHotkey } from "./useClipboardHotkeys";

const base = {
  batchMode: false,
  trashFilter: false,
  selectedId: 1 as number | null,
  selectedCount: 0,
};

describe("resolveListHotkey", () => {
  it("pastes on Enter in the default list", () => {
    expect(resolveListHotkey({ key: "Enter", altKey: false, ctrlKey: false, metaKey: false }, base)).toBe(
      "paste",
    );
  });

  it("toggles batch selection on Enter in batch mode instead of pasting", () => {
    expect(
      resolveListHotkey(
        { key: "Enter", altKey: false, ctrlKey: false, metaKey: false },
        { ...base, batchMode: true },
      ),
    ).toBe("toggle-batch-select");
  });

  it("restores on Enter in trash", () => {
    expect(
      resolveListHotkey(
        { key: "Enter", altKey: false, ctrlKey: false, metaKey: false },
        { ...base, trashFilter: true },
      ),
    ).toBe("restore");
  });

  it("batch-deletes on Delete when rows are checked", () => {
    expect(
      resolveListHotkey(
        { key: "Delete", altKey: false, ctrlKey: false, metaKey: false },
        { ...base, batchMode: true, selectedCount: 3 },
      ),
    ).toBe("batch-delete");
  });

  it("deletes the focused row on Delete outside batch mode", () => {
    expect(
      resolveListHotkey({ key: "Delete", altKey: false, ctrlKey: false, metaKey: false }, base),
    ).toBe("delete");
  });

  it("does not plain-paste with Alt+V in batch mode", () => {
    expect(
      resolveListHotkey(
        { key: "v", altKey: true, ctrlKey: false, metaKey: false },
        { ...base, batchMode: true },
      ),
    ).toBeNull();
  });
});
