import { describe, it, expect } from "vitest";
import { revealWindowAfterViewSwap } from "./revealWindow";

describe("revealWindowAfterViewSwap", () => {
  it("swaps the Vue view and flushes before showing the native window", async () => {
    const order: string[] = [];
    await revealWindowAfterViewSwap(
      () => {
        order.push("swap");
      },
      {
        unminimize: async () => {
          order.push("unminimize");
        },
        show: async () => {
          order.push("show");
        },
        setFocus: async () => {
          order.push("focus");
        },
      },
      async () => {
        order.push("flush");
      },
    );
    expect(order).toEqual(["swap", "flush", "unminimize", "show", "focus"]);
  });
});
