<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.features.title') }}</div>
    <p class="section-hint">{{ $t('settings.features.hint') }}</p>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.features.tags') }}</div>
        <div class="setting-desc">{{ $t('settings.features.tagsDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.features.tags"
        :aria-label="$t('settings.features.tags')"
        @update:model-value="(v: boolean) => setFeature('tags', v)"
      />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.features.batch') }}</div>
        <div class="setting-desc">{{ $t('settings.features.batchDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.features.batch"
        :aria-label="$t('settings.features.batch')"
        @update:model-value="(v: boolean) => setFeature('batch', v)"
      />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.features.sync') }}</div>
        <div class="setting-desc">{{ $t('settings.features.syncDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.features.sync"
        :aria-label="$t('settings.features.sync')"
        @update:model-value="(v: boolean) => setFeature('sync', v)"
      />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.features.stats') }}</div>
        <div class="setting-desc">{{ $t('settings.features.statsDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.features.stats"
        :aria-label="$t('settings.features.stats')"
        @update:model-value="(v: boolean) => setFeature('stats', v)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import type { FeatureId } from "../../features/capabilities";
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
