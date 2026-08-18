import { describe, expect, it } from "vitest";
import { AI_MODELS_MAX, DEFAULT_AI_MODEL, normalizeAiModels } from "./aiModels";

describe("normalizeAiModels", () => {
  it("fills the default when both list and current are empty", () => {
    expect(normalizeAiModels([], "")).toEqual({
      models: [DEFAULT_AI_MODEL],
      current: DEFAULT_AI_MODEL,
    });
    expect(normalizeAiModels(undefined, undefined)).toEqual({
      models: [DEFAULT_AI_MODEL],
      current: DEFAULT_AI_MODEL,
    });
  });

  it("seeds a missing list from the current model (upgrade JSON)", () => {
    expect(normalizeAiModels(undefined, "deepseek-chat")).toEqual({
      models: ["deepseek-chat"],
      current: "deepseek-chat",
    });
    expect(normalizeAiModels([], "deepseek-chat")).toEqual({
      models: ["deepseek-chat"],
      current: "deepseek-chat",
    });
  });

  it("prepends the current model when it is not in the list", () => {
    expect(normalizeAiModels(["gpt-4o-mini"], "deepseek-chat")).toEqual({
      models: ["deepseek-chat", "gpt-4o-mini"],
      current: "deepseek-chat",
    });
  });

  it("selects the first model when current is empty", () => {
    expect(normalizeAiModels(["llama3", "qwen-plus"], "  ")).toEqual({
      models: ["llama3", "qwen-plus"],
      current: "llama3",
    });
  });

  it("trims blanks, drops non-strings, and dedupes case-sensitively", () => {
    expect(
      normalizeAiModels([" gpt-4o-mini ", "", "gpt-4o-mini", "GPT-4o-mini", 12, " llama3 "], "llama3"),
    ).toEqual({
      models: ["gpt-4o-mini", "GPT-4o-mini", "llama3"],
      current: "llama3",
    });
  });

  it("keeps the current model when capping an oversized list", () => {
    const models = Array.from({ length: AI_MODELS_MAX + 5 }, (_, i) => `m${i}`);
    const result = normalizeAiModels(models, "m24");
    expect(result.models).toHaveLength(AI_MODELS_MAX);
    expect(result.current).toBe("m24");
    expect(result.models).toContain("m24");
  });
});
