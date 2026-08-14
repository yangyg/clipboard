<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.system.title') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.system.autoStart') }}</div>
        <div class="setting-desc">{{ $t('settings.system.autoStartDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.auto_start"
        :aria-label="$t('settings.system.autoStart')"
        @update:model-value="(v: boolean) => update('auto_start', v)"
      />
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.system.minimizeToTray') }}</div>
        <div class="setting-desc">{{ $t('settings.system.minimizeToTrayDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.minimize_to_tray"
        :aria-label="$t('settings.system.minimizeToTray')"
        @update:model-value="(v: boolean) => update('minimize_to_tray', v)"
      />
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.system.language') }}</div>
        <div class="setting-desc">{{ $t('settings.system.languageDesc') }}</div>
      </div>
      <div class="segmented" role="radiogroup" :aria-label="$t('settings.system.language')">
        <button
          type="button"
          class="segment-btn"
          role="radio"
          :aria-checked="settings.language === 'zh-CN'"
          :class="{ selected: settings.language === 'zh-CN' }"
          @click="updateLanguage('zh-CN')"
        >
          {{ $t('settings.system.langZhCN') }}
        </button>
        <button
          type="button"
          class="segment-btn"
          role="radio"
          :aria-checked="settings.language === 'en-US'"
          :class="{ selected: settings.language === 'en-US' }"
          @click="updateLanguage('en-US')"
        >
          {{ $t('settings.system.langEnUS') }}
        </button>
        <button
          type="button"
          class="segment-btn"
          role="radio"
          :aria-checked="settings.language === 'system'"
          :class="{ selected: settings.language === 'system' }"
          @click="updateLanguage('system')"
        >
          {{ $t('settings.system.langSystem') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import { setLocale, resolveLocale } from "../../locales";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();

function updateLanguage(lang: "zh-CN" | "en-US" | "system") {
  update("language", lang);
  setLocale(resolveLocale(lang));
}
</script>
