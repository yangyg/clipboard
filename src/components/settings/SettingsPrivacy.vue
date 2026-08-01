<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.privacy.sensitiveTitle') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.privacy.autoDetect') }}</div>
        <div class="setting-desc">{{ $t('settings.privacy.autoDetectDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.enable_sensitive_detection"
        :aria-label="$t('settings.privacy.autoDetect')"
        @update:model-value="(v: boolean) => update('enable_sensitive_detection', v)"
      />
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.privacy.autoExpire') }}</div>
        <div class="setting-desc">{{ $t('settings.privacy.autoExpireDesc') }}</div>
      </div>
      <div class="slider-row">
        <input type="range" min="10" max="3600" step="10" :aria-label="$t('settings.privacy.autoExpire')" :aria-valuetext="$t('settings.privacy.autoExpireUnit', { minutes: Math.floor(settings.sensitive_auto_expire_seconds / 60) })" :value="settings.sensitive_auto_expire_seconds" @input="(e) => update('sensitive_auto_expire_seconds', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ $t('settings.privacy.autoExpireUnit', { minutes: Math.floor(settings.sensitive_auto_expire_seconds / 60) }) }}</span>
      </div>
    </div>
  </div>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.privacy.ignoreTitle') }}</div>
    <div class="ignore-list">
      <div v-for="app in settings.ignored_apps" :key="app" class="ignore-item">
        <span class="ignore-icon"><AppIcon name="monitor" :size="14" /></span>
        <span class="ignore-name">{{ app }}</span>
        <button type="button" class="ignore-remove" :aria-label="$t('settings.privacy.removeApp', { app })" @click="removeIgnoredApp(app)"><AppIcon name="close" :size="12" /></button>
      </div>
    </div>
    <div class="ignore-add-row">
      <input class="ignore-input" :aria-label="$t('settings.privacy.ignoreTitle')" :placeholder="$t('settings.privacy.ignorePlaceholder')" v-model="newIgnoredApp" @keydown.enter="addIgnoredApp" />
      <button type="button" class="ignore-add-btn" @click="addIgnoredApp"><AppIcon name="plus" :size="13" /> {{ $t('settings.privacy.ignoreAdd') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../composables/useSettings";
import { useToast } from "../../composables/useToast";
import AppIcon from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const { toast } = useToast();
const { t } = useI18n();

const newIgnoredApp = ref("");

function addIgnoredApp() {
  const name = newIgnoredApp.value.trim();
  if (!name) {
    toast(t('settings.privacy.ignoreEmpty'), "warning");
    return;
  }
  if (settings.ignored_apps.includes(name)) {
    toast(t('settings.privacy.ignoreDuplicate'), "warning");
    return;
  }
  const updated = [...settings.ignored_apps, name];
  settingsStore.updateSetting("ignored_apps", updated);
  newIgnoredApp.value = "";
}

function removeIgnoredApp(app: string) {
  const updated = settings.ignored_apps.filter((a) => a !== app);
  settingsStore.updateSetting("ignored_apps", updated);
}
</script>

<style scoped>
.ignore-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.ignore-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.ignore-item:hover {
  background: var(--bg-hover);
}

.ignore-icon {
  display: flex;
  color: var(--text-tertiary);
}

.ignore-name {
  flex: 1;
  font-size: var(--text-md);
  color: var(--text-secondary);
}

.ignore-remove {
  font-size: var(--text-md);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 3px;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.ignore-remove:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.ignore-add-row {
  display: flex;
  gap: var(--space-2);
}

.ignore-input {
  flex: 1;
  height: var(--btn-height-lg);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-3);
  font-size: var(--text-md);
  color: var(--text-primary);
  transition: border-color var(--transition-fast), background var(--transition-smooth);
}

.ignore-input:focus {
  border-color: var(--border-focus);
}

.ignore-add-btn {
  height: var(--btn-height-lg);
  padding: 0 var(--space-4);
  background: var(--accent);
  color: white;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.ignore-add-btn:hover {
  background: var(--accent-hover);
}
</style>
