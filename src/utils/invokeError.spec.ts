import { describe, it, expect } from "vitest";
import { humanizeInvokeError } from "./invokeError";

const t = (key: string, params?: Record<string, unknown>) => {
  if (key === "common.featureDisabled") return `off:${params?.name}`;
  if (key === "settings.features.tags") return "标签";
  if (key === "common.operationFailed") return "failed";
  return key;
};

describe("humanizeInvokeError", () => {
  it("maps feature-disabled Rust errors to a named capability toast", () => {
    expect(humanizeInvokeError("feature disabled: tags", t)).toBe("off:标签");
  });

  it("falls back to the generic copy for other errors", () => {
    expect(humanizeInvokeError(new Error("boom"), t)).toBe("failed");
  });
});
