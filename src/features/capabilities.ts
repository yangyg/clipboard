import type { FeatureFlags } from "../types";

/** Keep in sync with Rust `FeatureId` / `FeatureFlags`. */
export type FeatureId = keyof FeatureFlags;

export const FEATURE_DEFINITIONS = [
  { id: "tags", labelKey: "settings.features.tags", descKey: "settings.features.tagsDesc" },
  { id: "batch", labelKey: "settings.features.batch", descKey: "settings.features.batchDesc" },
  { id: "sync", labelKey: "settings.features.sync", descKey: "settings.features.syncDesc" },
  { id: "stats", labelKey: "settings.features.stats", descKey: "settings.features.statsDesc" },
  { id: "ai", labelKey: "settings.features.ai", descKey: "settings.features.aiDesc" },
] as const satisfies ReadonlyArray<{
  id: FeatureId;
  labelKey: string;
  descKey: string;
}>;

export const DEFAULT_FEATURES: FeatureFlags = {
  tags: true,
  batch: true,
  sync: true,
  stats: true,
  ai: true,
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
