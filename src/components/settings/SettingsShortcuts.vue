<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.shortcuts.title') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.shortcuts.globalShortcut') }}</div>
        <div class="setting-desc">{{ $t('settings.shortcuts.globalShortcutDesc') }}</div>
      </div>
      <button
        class="shortcut-btn"
        :class="{ recording: isRecording }"
        type="button"
        @click="emit('start-recording')"
        @keydown="emit('shortcut-keydown', $event)"
      >
        {{ isRecording ? $t('settings.shortcuts.pressShortcut') : settings.global_shortcut }}
      </button>
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.shortcuts.searchFocus') }}</div>
        <div class="setting-desc">{{ $t('settings.shortcuts.searchFocusDesc') }}</div>
      </div>
      <span class="kbd-display">/ {{ $t('common.or') }} Ctrl+K</span>
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.shortcuts.plainPaste') }}</div>
        <div class="setting-desc">{{ $t('settings.shortcuts.plainPasteDesc') }}</div>
      </div>
      <span class="kbd-display">Alt + V</span>
    </div>
  </div>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.shortcuts.behavior') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.shortcuts.autoHide') }}</div>
        <div class="setting-desc">{{ $t('settings.shortcuts.autoHideDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.auto_close_on_paste"
        :aria-label="$t('settings.shortcuts.autoHide')"
        @update:model-value="(v: boolean) => update('auto_close_on_paste', v)"
      />
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.shortcuts.defaultPasteMode') }}</div>
        <div class="setting-desc">{{ $t('settings.shortcuts.defaultPasteModeDesc') }}</div>
      </div>
      <div class="segmented">
        <button
          v-for="mode in PASTE_MODES"
          :key="mode.key"
          type="button"
          class="segment-btn"
          :class="{ selected: settings.default_paste_mode === mode.key }"
          @click="update('default_paste_mode', mode.key)"
        >
          {{ $t(mode.labelKey) }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import ToggleSwitch from "../ToggleSwitch.vue";

defineProps<{
  /** Whether the parent shell is currently recording a global shortcut. */
  isRecording: boolean;
}>();

const emit = defineEmits<{
  "start-recording": [];
  "shortcut-keydown": [event: KeyboardEvent];
}>();

const { settings, update } = useSettings();

const PASTE_MODES = [
  { key: "original", labelKey: "settings.shortcuts.pasteOriginal" },
  { key: "plain", labelKey: "settings.shortcuts.pastePlain" },
] as const;
</script>

<style scoped>
.shortcut-btn {
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 5px 12px;
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  color: var(--text-secondary);
  min-width: 140px;
  text-align: center;
  cursor: pointer;
  transition: background var(--transition-smooth), border-color var(--transition-smooth), color var(--transition-fast);
}

.shortcut-btn:hover {
  border-color: var(--accent);
  color: var(--text-primary);
}

.shortcut-btn.recording {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent-text);
  animation: pulse-border var(--animation-pulse) infinite;
}

@keyframes pulse-border {
  50% { opacity: 0.75; }
}
</style>
