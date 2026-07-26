<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.history.title') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.history.maxRecords') }}</div>
        <div class="setting-desc">{{ $t('settings.history.maxRecordsDesc') }}</div>
      </div>
      <div class="slider-row">
        <input type="range" min="100" max="10000" step="100" :aria-label="$t('settings.history.maxRecords')" :value="settings.max_records" @input="(e) => update('max_records', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ settings.max_records }}</span>
      </div>
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.history.retentionDays') }}</div>
        <div class="setting-desc">{{ $t('settings.history.retentionDaysDesc') }}</div>
      </div>
      <div class="slider-row">
        <input type="range" min="7" max="365" step="1" :aria-label="$t('settings.history.retentionDays')" :value="settings.retention_days" @input="(e) => update('retention_days', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ settings.retention_days }} {{ $t('common.days') }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";

const { settings, update } = useSettings();
</script>
