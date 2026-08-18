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
        <input
          type="range"
          min="0"
          max="3600"
          step="10"
          :aria-label="$t('settings.privacy.autoExpire')"
          :aria-valuetext="expireValueText"
          :value="settings.sensitive_auto_expire_seconds"
          @input="(e) => update('sensitive_auto_expire_seconds', Number((e.target as HTMLInputElement).value))"
        />
        <span class="slider-value expire-value">{{ expireValueText }}</span>
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
      <TextInput ref="ignoreInput" class="ignore-input" :aria-label="$t('settings.privacy.ignoreTitle')" :placeholder="$t('settings.privacy.ignorePlaceholder')" v-model="newIgnoredApp" @keydown.enter="addIgnoredApp" />
      <button type="button" class="btn btn-primary btn-lg" :disabled="addDisabled" @click="addIgnoredApp"><AppIcon name="plus" :size="13" /> {{ $t('settings.privacy.ignoreAdd') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../composables/useSettings";
import { useToast } from "../../composables/useToast";
import { sensitiveExpireDisplay } from "../../utils/sensitiveExpiry";
import AppIcon from "../icons/AppIcon.vue";
import TextInput from "../TextInput.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, settingsStore, update } = useSettings();
const { toast } = useToast();
const { t } = useI18n();

const expireValueText = computed(() => {
  const d = sensitiveExpireDisplay(settings.sensitive_auto_expire_seconds);
  switch (d.kind) {
    case "never":
      return t("settings.privacy.autoExpireNever");
    case "seconds":
      return t("settings.privacy.autoExpireSeconds", { seconds: d.seconds });
    case "minutes":
      return t("settings.privacy.autoExpireMinutes", { minutes: d.minutes });
    case "compound":
      return t("settings.privacy.autoExpireCompound", {
        minutes: d.minutes,
        seconds: d.seconds,
      });
    default: {
      const _exhaustive: never = d;
      return _exhaustive;
    }
  }
});

const newIgnoredApp = ref("");
const ignoreInput = ref<InstanceType<typeof TextInput> | null>(null);

/** Empty (after trim) blocks adding — the disabled button says so. */
const addDisabled = computed(() => newIgnoredApp.value.trim() === "");

/** Mirror backend `is_ignored_app`: lowercase, exe-extensionless basename
 *  is equivalent to the full string. */
function ignoredAppKey(entry: string): string {
  const base = entry.trim().toLowerCase().split(/[\\/]/).pop() ?? "";
  return base.endsWith(".exe") ? base.slice(0, -4) : base;
}

function isDuplicateIgnoredApp(name: string): boolean {
  const key = ignoredAppKey(name);
  const full = name.trim().toLowerCase();
  return settings.ignored_apps.some(
    (app) => ignoredAppKey(app) === key || app.trim().toLowerCase() === full,
  );
}

function addIgnoredApp() {
  const name = newIgnoredApp.value.trim();
  if (!name) {
    // Keyboard path (Enter) — button clicks are blocked by :disabled.
    toast(t('settings.privacy.ignoreEmpty'), "warning");
    return;
  }
  if (isDuplicateIgnoredApp(name)) {
    toast(t('settings.privacy.ignoreDuplicate'), "warning");
    return;
  }
  const updated = [...settings.ignored_apps, name];
  settingsStore.updateSetting("ignored_apps", updated);
  newIgnoredApp.value = "";
  // Keep the flow going: focus stays on the box for the next entry.
  ignoreInput.value?.focus();
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
  background: var(--accent-softer);
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
  border-radius: var(--radius-xs);
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

/* :deep — the input now lives inside the TextInput shell component. */
:deep(.ignore-input) {
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

:deep(.ignore-input:focus) {
  border-color: var(--border-focus);
}

.expire-value {
  min-width: 7.5em;
  white-space: nowrap;
}
</style>
