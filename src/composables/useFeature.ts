import { computed, type ComputedRef } from "vue";
import { useSettingsStore } from "../stores/settings";
import { isFeatureEnabled, type FeatureId } from "../features/capabilities";

/** Reactive capability flag from the settings store. */
export function useFeature(id: FeatureId): ComputedRef<boolean> {
  const settingsStore = useSettingsStore();
  return computed(() => isFeatureEnabled(settingsStore.settings.features, id));
}
