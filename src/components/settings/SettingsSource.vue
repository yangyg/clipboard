<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.source.title') }}</div>
    <div class="setting-desc source-desc">{{ $t('settings.source.desc') }}</div>
    <div class="source-list">
      <div v-for="item in settings.source_name_overrides" :key="item.exe_name" class="source-item">
        <span class="source-exe">{{ item.exe_name }}</span>
        <span class="source-arrow" aria-hidden="true">→</span>
        <span class="source-name">{{ item.display_name }}</span>
        <button type="button" class="source-remove" :aria-label="$t('settings.source.remove', { app: item.exe_name })" @click="removeOverride(item.exe_name)">
          <AppIcon name="close" :size="12" />
        </button>
      </div>
    </div>
    <div class="source-add-row">
      <input class="source-input" :aria-label="$t('settings.source.exeLabel')" :placeholder="$t('settings.source.exePlaceholder')" v-model="newExe" @keydown.enter="addOverride" />
      <input class="source-input" :aria-label="$t('settings.source.nameLabel')" :placeholder="$t('settings.source.namePlaceholder')" v-model="newName" @keydown.enter="addOverride" />
      <button type="button" class="source-add-btn" @click="addOverride">
        <AppIcon name="plus" :size="13" /> {{ $t('settings.source.add') }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../composables/useSettings";
import { useToast } from "../../composables/useToast";
import AppIcon from "../icons/AppIcon.vue";

const { settings, settingsStore } = useSettings();
const { toast } = useToast();
const { t } = useI18n();

const newExe = ref("");
const newName = ref("");

function addOverride() {
  const exe = newExe.value.trim();
  const name = newName.value.trim();
  if (!exe || !name) {
    toast(t('settings.source.empty'), "warning");
    return;
  }
  const dup = settings.source_name_overrides.some(
    (o) => o.exe_name.toLowerCase() === exe.toLowerCase(),
  );
  if (dup) {
    toast(t('settings.source.duplicate'), "warning");
    return;
  }
  const updated = [
    ...settings.source_name_overrides,
    { exe_name: exe, display_name: name },
  ];
  settingsStore.updateSetting("source_name_overrides", updated);
  newExe.value = "";
  newName.value = "";
}

function removeOverride(exe: string) {
  const updated = settings.source_name_overrides.filter((o) => o.exe_name !== exe);
  settingsStore.updateSetting("source_name_overrides", updated);
}
</script>

<style scoped>
.source-desc {
  margin: -4px 0 10px;
}

.source-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.source-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}

.source-item:hover {
  background: var(--accent-softer);
}

.source-exe {
  font-family: var(--font-mono, monospace);
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.source-arrow {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.source-name {
  flex: 1;
  font-size: var(--text-md);
  color: var(--text-primary);
}

.source-remove {
  font-size: var(--text-md);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: var(--radius-xs);
  transition: background var(--transition-fast), color var(--transition-fast);
}

.source-remove:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.source-add-row {
  display: flex;
  gap: var(--space-2);
}

.source-input {
  flex: 1;
  min-width: 0;
  height: var(--btn-height-lg);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-3);
  font-size: var(--text-md);
  color: var(--text-primary);
  transition: border-color var(--transition-fast), background var(--transition-smooth);
}

.source-input:focus {
  border-color: var(--border-focus);
}

.source-add-btn {
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
  flex-shrink: 0;
}

.source-add-btn:hover {
  background: var(--accent-hover);
}
</style>
