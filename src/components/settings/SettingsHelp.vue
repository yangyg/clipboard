<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.help.title') }}</div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="keyboard" :size="14" /> {{ $t('settings.help.invokePanel') }}</div>
      <div class="guide-text">{{ $t('settings.help.invokePanelText', { shortcut: settings.global_shortcut }) }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="clipboard" :size="14" /> {{ $t('settings.help.autoRecord') }}</div>
      <div class="guide-text">{{ $t('settings.help.autoRecordText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="paste" :size="14" /> {{ $t('settings.help.pasteToApp') }}</div>
      <div class="guide-text">{{ $t('settings.help.pasteToAppText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="search" :size="14" /> {{ $t('settings.help.searchFilter') }}</div>
      <div class="guide-text">{{ $t('settings.help.searchFilterText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="history" :size="14" /> {{ $t('settings.help.searchHistory') }}</div>
      <div class="guide-text">{{ $t('settings.help.searchHistoryText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="star" :size="14" /> {{ $t('settings.help.favoritePinTag') }}</div>
      <div class="guide-text">{{ $t('settings.help.favoritePinTagText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="shield" :size="14" /> {{ $t('settings.help.privacyProtection') }}</div>
      <div class="guide-text">{{ $t('settings.help.privacyProtectionText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="trash" :size="14" /> {{ $t('settings.help.trashCleanup') }}</div>
      <div class="guide-text">{{ $t('settings.help.trashCleanupText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="panel" :size="14" /> {{ $t('settings.help.appModes') }}</div>
      <div class="guide-text">{{ $t('settings.help.appModesText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="type" :size="14" /> {{ $t('settings.help.fontFamily') }}</div>
      <div class="guide-text">{{ $t('settings.help.fontFamilyText') }}</div>
    </div>

    <div class="guide-block">
      <div class="guide-heading"><AppIcon name="stats" :size="14" /> {{ $t('settings.help.dataUsage') }}</div>
      <div class="guide-text">{{ $t('settings.help.dataUsageText') }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import AppIcon from "../icons/AppIcon.vue";

const { settings } = useSettings();
</script>

<style scoped>
.guide-block {
  padding: 12px 14px;
  margin-bottom: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

.guide-heading {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}

.guide-heading .app-icon {
  color: var(--accent-text);
}

.guide-text {
  font-size: var(--text-md);
  line-height: 1.7;
  color: var(--text-secondary);
}

.guide-kbd {
  /* extends global .kbd */
  font-size: var(--text-sm);
  color: var(--text-primary);
}
</style>
