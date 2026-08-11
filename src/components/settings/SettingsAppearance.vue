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
    <div class="settings-section-title">{{ $t('settings.appearance.searchBar') }}</div>
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.appearance.searchBarTitle') }}</div>
        <div class="setting-desc">{{ $t('settings.appearance.searchBarDesc') }}</div>
      </div>
      <div class="segmented">
        <button
          v-for="sm in SEARCH_MODES"
          :key="sm.key"
          type="button"
          class="segment-btn"
          :class="{ selected: settings.search_mode === sm.key }"
          @click="update('search_mode', sm.key)"
        >
          {{ $t(sm.labelKey) }}
        </button>
      </div>
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
      <div>
        <div class="setting-label">{{ $t('settings.appearance.alwaysOnTop') }}</div>
        <div class="setting-desc">{{ $t('settings.appearance.alwaysOnTopDesc') }}</div>
      </div>
      <ToggleSwitch
        :model-value="settings.always_on_top"
        :aria-label="$t('settings.appearance.alwaysOnTop')"
        @update:model-value="(v: boolean) => update('always_on_top', v)"
      />
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
    <div class="setting-row">
      <div>
        <div class="setting-label">{{ $t('settings.appearance.fontFamily') }}</div>
        <div class="setting-desc">{{ $t('settings.appearance.fontFamilyDesc') }}</div>
      </div>
      <select
        class="font-select"
        :value="presetSelectValue"
        :aria-label="$t('settings.appearance.fontFamily')"
        @change="onPresetChange"
      >
        <option v-for="p in FONT_PRESETS" :key="p.key" :value="p.key">{{ $t(p.labelKey) }}</option>
        <option :value="SYSTEM_FONT_OPTION_KEY">{{ $t('settings.appearance.fontSystem') }}</option>
      </select>
    </div>
    <div v-if="showSystemSelect" class="setting-row">
      <div class="setting-label">{{ $t('settings.appearance.fontSystemTitle') }}</div>
      <select
        v-if="systemFontsLoaded"
        class="font-select"
        :value="currentSystemFontName"
        :aria-label="$t('settings.appearance.fontSystemTitle')"
        @change="onSystemFontChange"
      >
        <option v-for="name in systemFonts" :key="name" :value="name">{{ name }}</option>
      </select>
      <span v-else class="setting-desc">{{ $t('settings.appearance.fontLoading') }}</span>
    </div>
    <div class="setting-row">
      <div class="setting-label">{{ $t('settings.appearance.fontPreview') }}</div>
      <div class="font-preview" :style="{ fontFamily: currentStack }">{{ $t('settings.appearance.fontPreviewSample') }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../composables/useSettings";
import {
  FONT_PRESETS,
  SYSTEM_FONT_OPTION_KEY,
  isSystemFontValue,
  resolveFontStack,
  systemFontName,
} from "../../utils/fontPresets";
import { useToast } from "../../composables/useToast";
import { i18n } from "../../locales";
import { THEME_DEFINITIONS } from "../../utils/themeRegistry";
import AppIcon from "../icons/AppIcon.vue";
import ToggleSwitch from "../ToggleSwitch.vue";

const { settings, update } = useSettings();

// --- Font family ---------------------------------------------------------

const systemFonts = ref<string[]>([]);
const systemFontsLoaded = ref(false);
/** User opted into the "系统字体…" entry (no committed setting until a font is picked). */
const showSystemMode = ref(false);

const isSystemFontSelected = computed(() => isSystemFontValue(settings.font_family));
const presetSelectValue = computed(() =>
  isSystemFontSelected.value || showSystemMode.value ? SYSTEM_FONT_OPTION_KEY : settings.font_family,
);
const showSystemSelect = computed(() => isSystemFontSelected.value || showSystemMode.value);
const currentSystemFontName = computed(() => systemFontName(settings.font_family));
const currentStack = computed(() => resolveFontStack(settings.font_family));

// Load the OS font list as soon as the system-font section is shown (also on
// re-open when a system font is already active).
watch(
  showSystemSelect,
  (visible) => {
    if (visible) void loadSystemFonts();
  },
  { immediate: true },
);

async function loadSystemFonts() {
  if (systemFontsLoaded.value) return;
  try {
    systemFonts.value = await invoke<string[]>("get_system_fonts");
  } catch (e) {
    console.error("Failed to load system fonts:", e);
    useToast().toast(i18n.global.t("settings.appearance.fontLoadError"), "error");
  } finally {
    systemFontsLoaded.value = true;
  }
}

function onPresetChange(e: Event) {
  const value = (e.target as HTMLSelectElement).value;
  if (value === SYSTEM_FONT_OPTION_KEY) {
    showSystemMode.value = true;
    void loadSystemFonts();
    return;
  }
  showSystemMode.value = false;
  update("font_family", value);
}

function onSystemFontChange(e: Event) {
  update("font_family", `system:${(e.target as HTMLSelectElement).value}`);
}


type ThemeOption = (typeof THEME_DEFINITIONS)[number];
const THEMES = THEME_DEFINITIONS;

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

/** Search bar display modes — keep in sync with `settings.search_mode`. */
const SEARCH_MODES = [
  { key: "full", labelKey: "settings.appearance.searchFull" },
  { key: "icon", labelKey: "settings.appearance.searchIcon" },
  { key: "hidden", labelKey: "settings.appearance.searchHidden" },
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
.theme-handdrawn {
  background-color: #211e1b;
  background-image: radial-gradient(ellipse at 20% 10%, rgba(255, 246, 224, 0.08), transparent 46%),
    radial-gradient(ellipse at 80% 75%, rgba(255, 246, 224, 0.08), transparent 42%),
    radial-gradient(rgba(255, 246, 224, 0.12) 0.8px, transparent 1.2px),
    repeating-linear-gradient(103deg, transparent 0 9px, rgba(255, 246, 224, 0.06) 9px 10px, transparent 10px 18px),
    repeating-linear-gradient(12deg, transparent 0 13px, rgba(255, 246, 224, 0.06) 13px 14px, transparent 14px 26px);
  background-size: 100% 100%, 100% 100%, 8px 8px, 100% 100%, 100% 100%;
}
.theme-handdrawn-light {
  background-color: #faf5ea;
  background-image: radial-gradient(ellipse at 20% 10%, rgba(58, 38, 20, 0.08), transparent 46%),
    radial-gradient(ellipse at 80% 75%, rgba(58, 38, 20, 0.08), transparent 42%),
    radial-gradient(rgba(58, 38, 20, 0.12) 0.8px, transparent 1.2px),
    repeating-linear-gradient(103deg, transparent 0 9px, rgba(58, 38, 20, 0.05) 9px 10px, transparent 10px 18px),
    repeating-linear-gradient(12deg, transparent 0 13px, rgba(58, 38, 20, 0.05) 13px 14px, transparent 14px 26px);
  background-size: 100% 100%, 100% 100%, 8px 8px, 100% 100%, 100% 100%;
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-mono { background: linear-gradient(135deg, #121212, #0a0a0a); }
.theme-mono-light {
  background: linear-gradient(135deg, #ffffff, #f0f0f0);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-editorial {
  background-color: #242322;
  background-image: linear-gradient(90deg, transparent 0 18%, rgba(255, 255, 255, 0.12) 18% 18.8%, transparent 18.8% 100%),
    linear-gradient(135deg, #332f2b 0 48%, #9d5b3c 48% 52%, #332f2b 52%);
}
.theme-editorial-light {
  background-color: #f5efe3;
  background-image: linear-gradient(90deg, transparent 0 18%, rgba(39, 34, 29, 0.18) 18% 18.8%, transparent 18.8% 100%),
    linear-gradient(135deg, #f5efe3 0 48%, #b65d3b 48% 52%, #f5efe3 52%);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-sticker {
  background-color: #293038;
  background-image: radial-gradient(circle at 25% 28%, #f5c84b 0 14%, transparent 15%),
    radial-gradient(circle at 74% 68%, #ed795f 0 18%, transparent 19%),
    linear-gradient(135deg, transparent 0 45%, #87c8c1 45% 62%, transparent 62%);
}
.theme-sticker-light {
  background-color: #f7f0df;
  background-image: radial-gradient(circle at 25% 28%, #f5c84b 0 14%, transparent 15%),
    radial-gradient(circle at 74% 68%, #ed795f 0 18%, transparent 19%),
    linear-gradient(135deg, transparent 0 45%, #73b9b2 45% 62%, transparent 62%);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-flat {
  background: linear-gradient(90deg, #5b8cff 0 3px, #1a1d23 3px);
}
.theme-flat-light {
  background: linear-gradient(90deg, #2f6bff 0 3px, #ffffff 3px);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-pencil {
  background-color: #272220;
  background-image: linear-gradient(115deg, transparent 0 36%, #45c2a4 36% 44%, transparent 44% 100%),
    linear-gradient(115deg, transparent 0 62%, #8fd0f0 62% 70%, transparent 70% 100%),
    linear-gradient(115deg, transparent 0 88%, #eec980 88% 96%, transparent 96% 100%);
}
.theme-pencil-light {
  background-color: #faf4eb;
  background-image: linear-gradient(115deg, transparent 0 36%, #0f7a63 36% 44%, transparent 44% 100%),
    linear-gradient(115deg, transparent 0 62%, #1f7ab9 62% 70%, transparent 70% 100%),
    linear-gradient(115deg, transparent 0 88%, #a26a0e 88% 96%, transparent 96% 100%);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}
.theme-pixel {
  background-color: #1b1a3a;
  background-image: linear-gradient(90deg, #262352 0 25%, #1b1a3a 25% 50%, #191838 50% 50%, #191838 50% 75%, #262352 75%),
    linear-gradient(135deg, transparent 0 70%, #ffc83d 70% 84%, transparent 84%),
    radial-gradient(#ffc83d 3px 3px, transparent 4px) 70% 15% / 9px 9px,
    repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.04) 0 1px, transparent 1px 4px);
}
.theme-pixel-light {
  background-color: #f6f4ff;
  background-image: linear-gradient(90deg, #e4def6 0 25%, #f6f4ff 25% 50%, #efeafd 50% 50%, #efeafd 50% 75%, #e4def6 75%),
    linear-gradient(135deg, transparent 0 70%, #b07d00 70% 84%, transparent 84%),
    radial-gradient(#b07d00 3px 3px, transparent 4px) 70% 15% / 9px 9px,
    repeating-linear-gradient(0deg, rgba(42, 36, 80, 0.05) 0 1px, transparent 1px 4px);
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.06));
}

.theme-name {
  font-size: var(--text-sm);
  font-weight: 500;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

/* Font family select + live preview */
.font-select {
  height: var(--btn-height-sm);
  max-width: 14rem;
  padding: 0 var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-surface);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: inherit;
  cursor: pointer;
  outline: none;
  transition: border-color var(--transition-fast), color var(--transition-fast);
}

.font-select:hover,
.font-select:focus {
  border-color: var(--accent);
  color: var(--text-primary);
}

.font-preview {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  border: 1px dashed var(--border-default);
  border-radius: var(--radius-sm);
  font-size: var(--text-lg);
  color: var(--text-primary);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  text-align: right;
}
</style>
