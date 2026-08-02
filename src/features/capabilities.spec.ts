import { describe, it, expect } from "vitest";
import {
  DEFAULT_FEATURES,
  isFeatureEnabled,
  mergeFeatures,
} from "./capabilities";

describe("capabilities", () => {
  it("defaults all features on", () => {
    expect(DEFAULT_FEATURES).toEqual({
      tags: true,
      batch: true,
      sync: true,
      stats: true,
    });
  });

  it("mergeFeatures fills missing keys with true", () => {
    expect(mergeFeatures({ tags: false })).toEqual({
      tags: false,
      batch: true,
      sync: true,
      stats: true,
    });
    expect(mergeFeatures(undefined)).toEqual(DEFAULT_FEATURES);
  });

  it("isFeatureEnabled treats missing as on only via merge", () => {
    expect(isFeatureEnabled(DEFAULT_FEATURES, "tags")).toBe(true);
    expect(isFeatureEnabled({ ...DEFAULT_FEATURES, tags: false }, "tags")).toBe(false);
  });
});
