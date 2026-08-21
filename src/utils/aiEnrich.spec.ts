import { describe, expect, it } from "vitest";
import { makeRecord } from "../test/factories";
import { isOnDemandAiEnabled, isOnDemandAiRecord, onDemandAiActions } from "./aiEnrich";
import { DEFAULT_FEATURES } from "../features/capabilities";

const enabled = { enable_ai: true, features: { ...DEFAULT_FEATURES } };
const disabledRuntime = { enable_ai: false, features: { ...DEFAULT_FEATURES } };
const disabledCapability = { enable_ai: true, features: { ...DEFAULT_FEATURES, ai: false } };

describe("on-demand AI eligibility", () => {
  it("allows text/code/link and rejects image/file/sensitive/trashed", () => {
    expect(isOnDemandAiRecord(makeRecord({ content_type: "text" }))).toBe(true);
    expect(isOnDemandAiRecord(makeRecord({ content_type: "code" }))).toBe(true);
    expect(isOnDemandAiRecord(makeRecord({ content_type: "link" }))).toBe(true);
    expect(isOnDemandAiRecord(makeRecord({ content_type: "image" }))).toBe(false);
    expect(isOnDemandAiRecord(makeRecord({ content_type: "file" }))).toBe(false);
    expect(isOnDemandAiRecord(makeRecord({ is_sensitive: true }))).toBe(false);
    expect(isOnDemandAiRecord(makeRecord({ is_trashed: true }))).toBe(false);
  });

  it("requires both capability and runtime switches", () => {
    expect(isOnDemandAiEnabled(enabled)).toBe(true);
    expect(isOnDemandAiEnabled(disabledRuntime)).toBe(false);
    expect(isOnDemandAiEnabled(disabledCapability)).toBe(false);
  });

  it("omits tags when the tags capability is off", () => {
    expect(onDemandAiActions(makeRecord(), enabled)).toEqual(["summary", "tags"]);
    expect(
      onDemandAiActions(makeRecord(), { enable_ai: true, features: { ...DEFAULT_FEATURES, tags: false } }),
    ).toEqual(["summary"]);
    expect(onDemandAiActions(makeRecord({ content_type: "image" }), enabled)).toEqual([]);
    expect(onDemandAiActions(makeRecord(), disabledRuntime)).toEqual([]);
  });
});
