import { useSettingsStore } from "../stores/settings";
import type { Settings } from "../types";

/**
 * Shared accessor for the settings store, used by the settings section
 * subcomponents. Pinia unwraps the store's `settings` ref into a reactive
 * object, so `settings.foo` can be bound directly in templates.
 */
export function useSettings() {
  const settingsStore = useSettingsStore();
  const settings = settingsStore.settings;

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    settingsStore.updateSetting(key, value);
  }

  return { settingsStore, settings, update };
}
