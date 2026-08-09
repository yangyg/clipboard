<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.history.title') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.history.importSystemHistory') }}</div>
        <div class="setting-desc">{{ $t('settings.history.importSystemHistoryDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.import_system_history_on_start"
        :aria-label="$t('settings.history.importSystemHistory')"
        @update:model-value="(v: boolean) => update('import_system_history_on_start', v)"
      />
    </div>
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
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.history.clearHistory') }}</div>
        <div class="setting-desc">{{ $t('settings.history.clearHistoryDesc') }}</div>
      </div>
      <button class="btn btn-danger" @click="clearHistory"><AppIcon name="trash" :size="13" /> {{ $t('settings.history.clearHistoryBtn') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../composables/useSettings";
import { useClipboardStore } from "../../stores/clipboard";
import { useConfirm } from "../../composables/useConfirm";
import { useToast } from "../../composables/useToast";
import AppIcon from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();
const clipboardStore = useClipboardStore();
const { confirm } = useConfirm();
const { toast } = useToast();
const { t } = useI18n();

async function clearHistory() {
  const ok = await confirm({
    title: t('confirm.clearHistoryTitle'),
    message: t('confirm.clearHistoryMsg'),
    confirmText: t('confirm.clearHistoryConfirm'),
    cancelText: t('common.cancel'),
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke("clear_history");
    await clipboardStore.loadRecords();
    toast(t('confirm.historyCleared'), "success");
  } catch (e) {
    console.error("Clear history failed:", e);
    toast(t('confirm.clearHistoryFailed'), "error");
  }
}
</script>
