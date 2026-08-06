<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.features.title') }}</div>
    <p class="section-hint">{{ $t('settings.features.hint') }}</p>

    <div v-for="feature in FEATURE_DEFINITIONS" :key="feature.id" class="setting-row">
      <div>
        <div class="setting-label">{{ $t(feature.labelKey) }}</div>
        <div class="setting-desc">{{ $t(feature.descKey) }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.features[feature.id]"
        :aria-label="$t(feature.labelKey)"
        @update:model-value="(v: boolean) => setFeature(feature.id, v)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import { FEATURE_DEFINITIONS, type FeatureId } from "../../features/capabilities";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();
const clipboardStore = useClipboardStore();

function setFeature(id: FeatureId, enabled: boolean) {
  update("features", { ...settings.features, [id]: enabled });
  if (id === "batch" && !enabled) {
    if (clipboardStore.batchMode) clipboardStore.toggleBatchMode();
    clipboardStore.selectedIds = new Set();
  }
  if (id === "tags" && !enabled) {
    if (clipboardStore.activeTag) {
      clipboardStore.filterByTag(null);
    }
    clipboardStore.tags = [];
    clipboardStore.reloadList();
  }
}
</script>

<style scoped>
.section-hint {
  margin: 0 0 12px;
  font-size: var(--text-md);
  color: var(--text-secondary);
  line-height: 1.45;
}
</style>
