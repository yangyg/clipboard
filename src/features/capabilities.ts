import { computed, type ComputedRef } from "vue";
import { useSettingsStore } from "../stores/settings";
import type { FeatureFlags } from "../types";

/** Keep in sync with Rust `FeatureId` / `FeatureFlags`. */
export type FeatureId = keyof FeatureFlags;

export const DEFAULT_FEATURES: FeatureFlags = {
  tags: true,
  batch: true,
  sync: true,
  stats: true,
};

/** Settings nav keys that disappear when the capability is off. */
export const FEATURE_SETTINGS_SECTIONS: Partial<Record<FeatureId, string>> = {
  tags: "tags",
  sync: "sync",
  stats: "stats",
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

/** Reactive capability flag from the settings store. */
export function useFeature(id: FeatureId): ComputedRef<boolean> {
  const settingsStore = useSettingsStore();
  return computed(() => isFeatureEnabled(settingsStore.settings.features, id));
}
