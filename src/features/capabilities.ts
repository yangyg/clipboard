import type { FeatureFlags } from "../types";

/** Keep in sync with Rust `FeatureId` / `FeatureFlags`. */
export type FeatureId = keyof FeatureFlags;

export const DEFAULT_FEATURES: FeatureFlags = {
  tags: true,
  batch: true,
  sync: true,
  stats: true,
};

export function mergeFeatures(partial?: Partial<FeatureFlags> | null): FeatureFlags {
  return {
    ...DEFAULT_FEATURES,
    ...(partial ?? {}),
  };
}

export function isFeatureEnabled(features: FeatureFlags, id: FeatureId): boolean {
  return features[id] !== false;
}
