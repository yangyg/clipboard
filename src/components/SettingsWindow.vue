<template>
  <div class="settings-overlay" tabindex="-1" @keydown.esc="onOverlayEsc">
    <div class="settings-window panel-surface">
      <!-- Header -->
      <div class="settings-header" :class="{ 'with-chrome': isWindowMode }" data-tauri-drag-region>
        <span class="settings-title"><AppIcon name="settings" :size="15" /> {{ $t('settings.title') }}</span>
        <div v-if="isWindowMode" class="settings-header-right">
          <WindowControls />
        </div>
      </div>

      <div class="settings-main">
        <!-- Nav: back stays pinned; sections scroll independently -->
        <nav class="settings-nav">
          <div class="nav-back-bar">
            <button type="button" class="nav-item nav-back" :title="$t('settings.back')" :aria-label="$t('settings.back')" @click="emit('close')">
              <span class="nav-icon"><AppIcon name="back" :size="15" /></span>
              <span class="nav-label">{{ $t('settings.back') }}</span>
            </button>
            <div class="nav-divider" aria-hidden="true"></div>
          </div>
          <div class="nav-scroll">
            <template v-for="group in visibleGroups" :key="group.key">
              <div class="nav-group-title">{{ $t(group.labelKey) }}</div>
              <button
                v-for="section in group.sections"
                :key="section.key"
                type="button"
                class="nav-item"
                :class="{ active: activeSection === section.key }"
                :title="$t(section.labelKey)"
                :aria-label="$t(section.labelKey)"
                @click="activeSection = section.key"
              >
                <span class="nav-icon"><AppIcon :name="section.icon" :size="15" /></span>
                <span class="nav-label">{{ $t(section.labelKey) }}</span>
              </button>
            </template>
          </div>
        </nav>

        <!-- Body: keyed wrapper remounts on section switch so the fade-in
             animation runs each time. Leave is an instant cut (a one-way
             enter is enough to anchor "content changed"). Pure CSS animation,
             not a JS <Transition>, matching the cold-start-safe list pattern. -->
        <div class="settings-body">
          <div class="settings-section-fade" :key="activeSection">
            <SettingsShortcuts
              v-if="activeSection === 'shortcuts'"
              :is-recording="isRecordingShortcut"
              @start-recording="startShortcutRecording"
              @shortcut-keydown="onShortcutKeydown"
            />
            <SettingsAppearance v-else-if="activeSection === 'appearance'" />
            <SettingsHistory v-else-if="activeSection === 'history'" />
            <SettingsSource v-else-if="activeSection === 'source'" />
            <SettingsTags v-else-if="activeSection === 'tags'" />
            <SettingsPrivacy v-else-if="activeSection === 'privacy'" />
            <SettingsFeatures v-else-if="activeSection === 'features'" />
            <SettingsStats v-else-if="activeSection === 'stats'" />
            <SettingsData v-else-if="activeSection === 'data'" />
            <SettingsSync v-else-if="activeSection === 'sync'" />
            <SettingsSystem v-else-if="activeSection === 'system'" />
            <SettingsHelp v-else-if="activeSection === 'help'" />
            <SettingsAbout v-else-if="activeSection === 'about'" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useSettingsStore } from "../stores/settings";
import { useClipboardStore } from "../stores/clipboard";
import { isFeatureEnabled, type FeatureId } from "../features/capabilities";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";
import WindowControls from "./WindowControls.vue";
import SettingsShortcuts from "./settings/SettingsShortcuts.vue";
import SettingsAppearance from "./settings/SettingsAppearance.vue";
import SettingsHistory from "./settings/SettingsHistory.vue";
import SettingsSource from "./settings/SettingsSource.vue";
import SettingsTags from "./settings/SettingsTags.vue";
import SettingsPrivacy from "./settings/SettingsPrivacy.vue";
import SettingsFeatures from "./settings/SettingsFeatures.vue";
import SettingsStats from "./settings/SettingsStats.vue";
import SettingsData from "./settings/SettingsData.vue";
import SettingsSync from "./settings/SettingsSync.vue";
import SettingsSystem from "./settings/SettingsSystem.vue";
import SettingsHelp from "./settings/SettingsHelp.vue";
import SettingsAbout from "./settings/SettingsAbout.vue";

const emit = defineEmits<{ close: [] }>();
const props = defineProps<{
  initialSection?: string;
}>();
const settingsStore = useSettingsStore();
const clipboardStore = useClipboardStore();
const settings = settingsStore.settings;
const isWindowMode = computed(() => settings.app_mode === "window");

const activeSection = ref(props.initialSection ?? "appearance");
const isRecordingShortcut = ref(false);

type GroupId = "general" | "content" | "privacySystem" | "dataSync" | "infoSupport";

const GROUPS: { key: GroupId; labelKey: string }[] = [
  { key: "general", labelKey: "settings.navGroup.general" },
  { key: "content", labelKey: "settings.navGroup.content" },
  { key: "privacySystem", labelKey: "settings.navGroup.privacySystem" },
  { key: "dataSync", labelKey: "settings.navGroup.dataSync" },
  { key: "infoSupport", labelKey: "settings.navGroup.infoSupport" },
];

const ALL_SECTIONS: {
  key: string;
  icon: AppIconName;
  labelKey: string;
  group: GroupId;
  feature?: FeatureId;
}[] = [
  // 通用
  { key: "appearance", icon: "palette", labelKey: "settings.nav.appearance", group: "general" },
  { key: "shortcuts", icon: "keyboard", labelKey: "settings.nav.shortcuts", group: "general" },
  { key: "features", icon: "component", labelKey: "settings.nav.features", group: "general" },
  // 内容
  { key: "tags", icon: "tag", labelKey: "settings.nav.tags", group: "content", feature: "tags" },
  { key: "history", icon: "history", labelKey: "settings.nav.history", group: "content" },
  { key: "source", icon: "monitor", labelKey: "settings.nav.source", group: "content" },
  // 隐私与系统
  { key: "privacy", icon: "shield", labelKey: "settings.nav.privacy", group: "privacySystem" },
  { key: "system", icon: "settings2", labelKey: "settings.nav.system", group: "privacySystem" },
  // 数据与同步
  { key: "data", icon: "package", labelKey: "settings.nav.data", group: "dataSync" },
  { key: "sync", icon: "cloud", labelKey: "settings.nav.sync", group: "dataSync", feature: "sync" },
  // 信息与支持
  { key: "stats", icon: "stats", labelKey: "settings.nav.stats", group: "infoSupport", feature: "stats" },
  { key: "help", icon: "help", labelKey: "settings.nav.help", group: "infoSupport" },
  { key: "about", icon: "info", labelKey: "settings.nav.about", group: "infoSupport" },
];

const visibleSections = computed(() =>
  ALL_SECTIONS.filter(
    (s) => !s.feature || isFeatureEnabled(settings.features, s.feature),
  ),
);

const visibleGroups = computed(() =>
  GROUPS.map((group) => ({
    ...group,
    sections: ALL_SECTIONS.filter(
      (s) =>
        s.group === group.key &&
        (!s.feature || isFeatureEnabled(settings.features, s.feature)),
    ),
  })).filter((group) => group.sections.length > 0),
);

watch(
  visibleSections,
  (sections) => {
    if (!sections.some((s) => s.key === activeSection.value)) {
      activeSection.value = sections[0]?.key ?? "appearance";
    }
  },
  { immediate: true },
);

const KEY_ALIASES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Esc",
};

function startShortcutRecording() {
  isRecordingShortcut.value = true;
}

function stopShortcutRecording() {
  isRecordingShortcut.value = false;
}

function onOverlayEsc() {
  if (isRecordingShortcut.value) {
    stopShortcutRecording();
    return;
  }
  emit("close");
}

function normalizeKey(key: string): string | null {
  if (["Control", "Shift", "Alt", "Meta", "OS"].includes(key)) return null;
  if (KEY_ALIASES[key]) return KEY_ALIASES[key];
  if (key.length === 1) return key.toUpperCase();
  if (key.startsWith("Key") && key.length === 4) return key.slice(3);
  if (key.startsWith("Digit") && key.length === 6) return key.slice(5);
  return key;
}

function onShortcutKeydown(e: KeyboardEvent) {
  if (!isRecordingShortcut.value) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    stopShortcutRecording();
    return;
  }

  // Enter / Escape alone must not become the shortcut
  if (e.key === "Enter" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
    return;
  }

  const key = normalizeKey(e.key);
  if (!key) return;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);

  // Require at least one modifier for a global shortcut
  if (parts.length < 2) return;

  const combo = parts.join("+");
  settingsStore.updateSetting("global_shortcut", combo);
  stopShortcutRecording();
}

function onWindowKeydown(e: KeyboardEvent) {
  if (!isRecordingShortcut.value) return;
  onShortcutKeydown(e);
}

onMounted(() => {
  clipboardStore.loadStats();
  window.addEventListener("keydown", onWindowKeydown, true);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onWindowKeydown, true);
});
</script>

<style scoped>
.settings-overlay {
  position: fixed;
  inset: 0;
  background: transparent;
  display: flex;
  z-index: 200;
}

.settings-window {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.settings-header.with-chrome {
  padding: 0 0 0 16px;
  height: 38px;
  min-height: 38px;
}

.settings-header-right {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 100%;
  -webkit-app-region: no-drag;
}

.settings-header.with-chrome .settings-header-right {
  margin-right: 0;
}

.settings-title {
  font-size: var(--text-base);
  font-weight: 600;
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.settings-main {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.settings-nav {
  width: 180px;
  padding: 0 var(--space-3);
  background: var(--bg-elevated);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-height: 0;
  overflow: hidden;
  transition: background var(--transition-smooth), border-color var(--transition-smooth);
}

.nav-back-bar {
  flex-shrink: 0;
  padding: 12px 0 0;
  background: var(--bg-elevated);
}

.nav-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* Extend to the nav edges so the scrollbar hugs the right border, then
     restore the horizontal inset with matching padding. */
  margin: 0 calc(-1 * var(--space-3));
  padding: 0 var(--space-3) 16px;
}

.nav-back {
  color: var(--text-secondary);
  font-weight: 500;
}

.nav-back:hover {
  color: var(--text-primary);
}

.nav-divider {
  height: 1px;
  margin: 8px var(--space-2);
  background: var(--border-subtle);
  flex-shrink: 0;
}

.nav-group-title {
  padding: 14px var(--space-2) 4px;
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin: 0;
  padding: 7px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  font: inherit;
  font-size: var(--text-md);
  line-height: 1;
  text-align: left;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent-text);
}

.nav-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  color: inherit;
  line-height: 0;
}

.nav-label {
  line-height: 1.2;
}

.settings-body {
  flex: 1;
  padding: 20px 24px;
  background: var(--bg-surface);
  overflow-y: auto;
  min-width: 0;
}

/* One-way fade when switching sections — the enter anchors "content just
   changed". Keyed on the wrapper so the animation reruns per switch. */
.settings-section-fade {
  animation: settings-section-in var(--transition-fast) ease;
}

@keyframes settings-section-in {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

:global(body.anim-disabled) .settings-section-fade {
  animation: none;
}

@media (prefers-reduced-motion: reduce) {
  .settings-section-fade {
    animation: none;
  }
}

@media (max-width: 720px) {
  .settings-nav {
    width: 56px;
  }

  .nav-back-bar {
    padding: 8px 0 0;
  }

  .nav-scroll {
    padding: 0 0 12px;
  }

  .nav-group-title {
    display: none;
  }

  .settings-nav .nav-item {
    justify-content: center;
    padding: 10px 8px;
  }

  .settings-nav .nav-label {
    display: none;
  }
}
</style>
