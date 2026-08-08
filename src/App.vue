<template>
  <div class="app-root">
    <!-- Floating mode: keep panel mounted (v-show) to avoid full remount cost -->
    <template v-if="!isWindowMode">
      <FloatingPanel v-show="panelVisible && !settingsVisible" :settings-visible="settingsVisible" @close="hidePanel" @openSettings="openSettings" />
      <SettingsWindow v-if="settingsVisible" :initial-section="settingsInitialSection" @close="closeSettings" />
    </template>
    <!-- Window mode: panel always visible, settings replaces panel -->
    <template v-else>
      <SettingsWindow v-if="settingsVisible" :initial-section="settingsInitialSection" @close="closeSettings" />
      <WindowApp v-else-if="panelVisible" @openSettings="openSettings" />
    </template>
    <ToastHost />
    <ConfirmDialog />
    <WelcomeDialog
      :open="welcomeOpen"
      :shortcut="settings.global_shortcut"
      @complete="completeOnboarding"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import FloatingPanel from "./components/FloatingPanel.vue";
import WindowApp from "./components/WindowApp.vue";
import SettingsWindow from "./components/SettingsWindow.vue";
import ToastHost from "./components/ToastHost.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import WelcomeDialog from "./components/WelcomeDialog.vue";
import { useClipboardStore } from "./stores/clipboard";
import { useSettingsStore } from "./stores/settings";
import { storeToRefs } from "pinia";
import { useConfirm } from "./composables/useConfirm";
import { useClipboardEvents } from "./composables/useClipboardEvents";
import { setLocale, resolveLocale } from "./locales";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { settings } = storeToRefs(settingsStore);
const { current: confirmOpen, settle: settleConfirm } = useConfirm();

const panelVisible = ref(false);
const settingsVisible = ref(false);
const settingsInitialSection = ref<string | undefined>(undefined);
const welcomeOpen = ref(false);
/** Avoid full get_records on every focus if list is fresh enough. */
let lastPanelReloadAt = 0;
const PANEL_RELOAD_TTL_MS = 30_000;

const appWindow = getCurrentWindow();
const isWindowMode = computed(() => settings.value.app_mode === "window");

async function applyAppMode() {
  const mode = isWindowMode.value ? "window" : "floating";
  try {
    await invoke("switch_app_mode", { mode });
  } catch (e) {
    console.error("[App] switch_app_mode failed:", e);
  }
  // Sync state after Rust command completes to avoid window flash
  if (isWindowMode.value) {
    panelVisible.value = true;
    settingsVisible.value = false;
  }
}

async function reloadPanelIfNeeded(force = false) {
  const now = Date.now();
  const stale = now - lastPanelReloadAt > PANEL_RELOAD_TTL_MS;
  if (force || stale || clipboardStore.records.length === 0) {
    lastPanelReloadAt = now;
    await clipboardStore.loadRecords();
  }
}

async function showPanel() {
  panelVisible.value = true;
  settingsVisible.value = false;
  // Snapshot previous FG before we steal focus (backup for non-Rust show paths).
  try {
    await invoke("capture_paste_target");
  } catch (e) {
    console.warn("[App] capture_paste_target failed:", e);
  }
  // Show the window BEFORE loading records: the list's <Transition mode="out-in">
  // relies on requestAnimationFrame, which never fires in a hidden WebView2
  // window — starting a transition while hidden leaves the list permanently
  // unmounted (blank list on cold start).
  await appWindow.show();
  await appWindow.setFocus();
  await reloadPanelIfNeeded(false);
}

async function hidePanel() {
  if (isWindowMode.value) {
    // Window mode: Rust already hid/minimized the window on the toggle-panel
    // false event. Re-showing it here would undo the tray "hide" immediately.
    panelVisible.value = true;
    settingsVisible.value = false;
    return;
  }
  // Settle any open confirm so its promise does not hang across hide/show.
  if (confirmOpen.value) settleConfirm(false);
  panelVisible.value = false;
  settingsVisible.value = false;
  await appWindow.hide();
}

function closeSettings() {
  settingsVisible.value = false;
  panelVisible.value = true;
  settingsInitialSection.value = undefined;
}

function completeOnboarding() {
  if (!welcomeOpen.value) return;
  welcomeOpen.value = false;
  settingsStore.updateSetting("onboarding_completed", true);
}

async function openSettings(section?: string) {
  settingsInitialSection.value = section;
  if (isWindowMode.value) {
    panelVisible.value = true;
    settingsVisible.value = true;
  } else {
    // Keep FloatingPanel mounted (v-show); only swap visibility with settings.
    panelVisible.value = true;
    settingsVisible.value = true;
  }
  await appWindow.show();
  await appWindow.setFocus();
}

// Rust→frontend event wiring (owns its own onMounted/onUnmounted listener
// lifecycle, so dev HMR cannot leak duplicate listeners).
useClipboardEvents({
  appWindow,
  isWindowMode: () => isWindowMode.value,
  panelVisible,
  settingsVisible,
  showPanel,
  hidePanel,
  openSettings,
});

onMounted(async () => {
  // Load settings
  await settingsStore.loadSettings();
  setLocale(resolveLocale(settings.value.language));
  await applyAppMode();

  // showPanel() loads records once (avoid a duplicate get_records on cold start)
  await clipboardStore.loadTags();
  lastPanelReloadAt = 0; // force first load
  await showPanel();

  if (!settings.value.onboarding_completed) {
    welcomeOpen.value = true;
  }

  watch(
    () => settings.value.app_mode,
    async () => {
      await applyAppMode();
      await appWindow.show();
      await appWindow.setFocus();
    }
  );

  watch(
    () => settings.value.language,
    (lang) => {
      setLocale(resolveLocale(lang));
    }
  );
});
</script>

<style scoped>
.app-root {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  /* Stack floating panel + settings so swaps don't reflow layout. */
  display: grid;
}
.app-root > * {
  grid-area: 1 / 1;
  min-width: 0;
  min-height: 0;
}
</style>
