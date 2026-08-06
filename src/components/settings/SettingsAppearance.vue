<template>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.appearance.theme') }}</div>
    <div class="theme-cards" role="radiogroup" :aria-label="$t('settings.appearance.theme')">
      <div
        v-for="(t, idx) in THEMES"
        :key="t.key"
        class="theme-card"
        role="radio"
        :data-theme="t.key"
        :aria-checked="settings.theme === t.key"
        :aria-label="$t(t.labelKey)"
        :tabindex="settings.theme === t.key ? 0 : -1"
        :class="{ selected: settings.theme === t.key }"
        @click="update('theme', t.key)"
        @keydown.enter.prevent="update('theme', t.key)"
        @keydown.space.prevent="update('theme', t.key)"
        @keydown.right.prevent="focusTheme(THEMES, idx + 1)"
        @keydown.left.prevent="focusTheme(THEMES, idx - 1)"
        @keydown.down.prevent="focusTheme(THEMES, idx + 1)"
        @keydown.up.prevent="focusTheme(THEMES, idx - 1)"
      >
        <div class="theme-preview" :class="`theme-${t.key}`" aria-hidden="true"></div>
        <div class="theme-name"><AppIcon :name="t.icon" :size="13" /> {{ $t(t.labelKey) }}</div>
      </div>
    </div>
  </div>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.appearance.appMode') }}</div>
    <div class="mode-grid" role="radiogroup" :aria-label="$t('settings.appearance.appMode')">
      <button
        v-for="mode in APP_MODES"
        :key="mode.key"
        type="button"
        class="mode-card"
        role="radio"
        :aria-checked="settings.app_mode === mode.key"
        :class="{ selected: settings.app_mode === mode.key }"
        @click="update('app_mode', mode.key)"
      >
        <span class="mode-icon"><AppIcon :name="mode.icon" :size="18" /></span>
        <span class="mode-title">{{ $t(mode.labelKey) }}</span>
        <span class="mode-desc">{{ $t(mode.descKey) }}</span>
      </button>
    </div>
  </div>
  <div class="settings-section">
    <div class="settings-section-title">{{ $t('settings.appearance.panelAppearance') }}</div>
    <div class="setting-row">
      <div class="setting-label">{{ $t('settings.appearance.cornerRadius') }}</div>
      <div class="slider-row">
        <input type="range" min="0" max="40" :aria-label="$t('settings.appearance.cornerRadius')" :aria-valuetext="`${settings.panel_radius}px`" :value="settings.panel_radius" @input="(e) => update('panel_radius', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ settings.panel_radius }}px</span>
      </div>
    </div>
    <div class="setting-row">
        <div class="setting-label">{{ $t('settings.appearance.opacity') }}</div>
      <div class="slider-row">
        <input type="range" min="60" max="100" :aria-label="$t('settings.appearance.opacity')" :aria-valuetext="`${settings.panel_opacity}%`" :value="settings.panel_opacity" @input="(e) => update('panel_opacity', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ settings.panel_opacity }}%</span>
      </div>
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.appearance.blur') }}</div>
        <div class="setting-desc">{{ $t('settings.appearance.blurDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.enable_blur"
        :aria-label="$t('settings.appearance.blur')"
        @update:model-value="(v: boolean) => update('enable_blur', v)"
      />
    </div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.appearance.blurStrength') }}</div>
        <div class="setting-desc">{{ $t('settings.appearance.blurStrengthDesc') }}</div>
      </div>
      <div class="slider-row">
        <input
          type="range"
          min="30"
          max="80"
          :disabled="!settings.enable_blur"
          :aria-label="$t('settings.appearance.blurStrength')"
          :aria-valuetext="`${settings.blur_strength}%`"
          :value="settings.blur_strength"
          @input="(e) => update('blur_strength', Number((e.target as HTMLInputElement).value))"
        />
        <span class="slider-value">{{ settings.blur_strength }}%</span>
      </div>
    </div>
    <div class="setting-row">
      <div class="setting-label">{{ $t('settings.appearance.animation') }}</div>
      <ToggleSwitch
        :model-value="settings.enable_animation"
        :aria-label="$t('settings.appearance.animation')"
        @update:model-value="(v: boolean) => update('enable_animation', v)"
      />
    </div>
    <div class="setting-row">
      <div class="setting-label">{{ $t('settings.appearance.fontSize') }}</div>
      <div class="slider-row">
        <input type="range" min="11" max="22" :aria-label="$t('settings.appearance.fontSize')" :aria-valuetext="`${settings.font_size}px`" :value="settings.font_size" @input="(e) => update('font_size', Number((e.target as HTMLInputElement).value))" />
        <span class="slider-value">{{ settings.font_size }}px</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettings } from "../../composables/useSettings";
import type { Settings } from "../../types";
import AppIcon, { type AppIconName } from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();

type ThemeOption = { key: Settings["theme"]; icon: AppIconName; labelKey: string };

const THEMES: ThemeOption[] = [
  { key: "dark", icon: "moon", labelKey: "settings.appearance.themeDark" },
  { key: "light", icon: "sun", labelKey: "settings.appearance.themeLight" },
  { key: "oled", icon: "circle", labelKey: "settings.appearance.themeOled" },
  { key: "dracula", icon: "sparkles", labelKey: "settings.appearance.themeDracula" },
  { key: "nord", icon: "zap", labelKey: "settings.appearance.themeNord" },
  { key: "sunset", icon: "star", labelKey: "settings.appearance.themeSunset" },
  { key: "dracula-light", icon: "sparkles", labelKey: "settings.appearance.themeDraculaLight" },
  { key: "nord-light", icon: "zap", labelKey: "settings.appearance.themeNordLight" },
  { key: "sunset-light", icon: "star", labelKey: "settings.appearance.themeSunsetLight" },
];

function focusTheme(items: readonly ThemeOption[], index: number) {
  const len = items.length;
  const next = ((index % len) + len) % len;
  const key = items[next].key;
  update("theme", key);
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLElement>(`.theme-card[data-theme="${key}"]`);
    el?.focus();
  });
}

const APP_MODES = [
  {
    key: "floating",
    icon: "panel" as AppIconName,
    labelKey: "settings.appearance.modeFloating",
    descKey: "settings.appearance.modeFloatingDesc",
  },
  {
    key: "window",
    icon: "window" as AppIconName,
    labelKey: "settings.appearance.modeWindow",
    descKey: "settings.appearance.modeWindowDesc",
  },
] as const;
</script>

<style scoped>
/* Theme cards */
.theme-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
  gap: 10px;
  margin-bottom: 16px;
}

.theme-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px;
  cursor: pointer;
  text-align: center;
  transition: border-color var(--transition-fast), background var(--transition-fast);
}

.theme-card:hover {
  border-color: var(--accent);
}

.theme-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.theme-card:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.theme-preview {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-sm);
  margin-bottom: var(--space-2);
}

.theme-dark { background: linear-gradient(135deg, var(--bg-surface), var(--bg-elevated)); }
.theme-light {
  background: linear-gradient(135deg, #ffffff, #f0f2f8);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-oled { background: #000000; }
.theme-dracula { background: linear-gradient(135deg, #282a36, #1e1f29); }
.theme-nord { background: linear-gradient(135deg, #2e3440, #20252e); }
.theme-sunset { background: linear-gradient(135deg, #29201a, #1c1512); }
.theme-dracula-light { background: linear-gradient(135deg, #faf7ff, #f3eefb); }
.theme-nord-light { background: linear-gradient(135deg, #f0f4f8, #e8edf3); }
.theme-sunset-light { background: linear-gradient(135deg, #fdf7ee, #f7efe4); }

.theme-name {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

/* Mode cards */
.mode-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.mode-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 14px;
  cursor: pointer;
  text-align: center;
  transition: border-color var(--transition-fast);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.mode-card:hover {
  border-color: var(--accent);
}

.mode-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.mode-icon {
  display: flex;
  color: var(--accent-text);
  line-height: 1;
}

.mode-title {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--text-primary);
}

.mode-desc {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  line-height: 1.4;
}
</style>
