<template>
  <div class="app-root">
    <!-- Floating mode: keep panel mounted (v-show) to avoid full remount cost -->
    <template v-if="!isWindowMode">
      <FloatingPanel v-show="panelVisible && !settingsVisible" @close="hidePanel" @openSettings="openSettings" />
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
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
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
import { isPasteFocusLock, setPasteFocusLock } from "./composables/pasteFocusLock";
import { useConfirm } from "./composables/useConfirm";
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
    panelVisible.value = true;
    settingsVisible.value = false;
    await appWindow.show();
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

// Track Tauri event listeners so they can be torn down on unmount. Without this,
// dev HMR re-runs onMounted and would register duplicate listeners that leak.
let unlisteners: Array<() => void> = [];
onUnmounted(() => {
  for (const off of unlisteners) off();
  unlisteners = [];
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

  // Reset (dev HMR re-runs onMounted); collected listeners are torn down by onUnmounted.
  unlisteners = [];

  // Listen for new clipboard records from Rust backend
  unlisteners.push(
    await listen<any>("clipboard-changed", (event) => {
      if (!clipboardStore.pauseCapture) {
        clipboardStore.onNewRecord(event.payload);
      }
    })
  );

  // Sensitive auto-expire deleted in Rust (periodic cleanup thread) → sync list
  unlisteners.push(
    await listen<number[]>("records-expired", (event) => {
      clipboardStore.removeExpiredFromList(event.payload ?? []);
      clipboardStore.scheduleLoadStats();
    })
  );

  // Listen for toggle-panel from Rust (Rust shows/hides window, we sync panelVisible)
  unlisteners.push(
    await listen<boolean>("toggle-panel", (event) => {
      if (isPasteFocusLock() && event.payload) {
        // Mid-paste / keep-open: sync flag only — never setFocus (would steal from target).
        panelVisible.value = true;
        return;
      }
      if (event.payload) {
        if (!panelVisible.value || settingsVisible.value) {
          showPanel();
        } else {
          // Already visible — still show/focus window without forcing reload
          void appWindow.show().then(() => appWindow.setFocus());
        }
      } else {
        if (panelVisible.value) {
          hidePanel();
        }
      }
    })
  );

  unlisteners.push(
    await listen<boolean>("paste-focus-lock", (event) => {
      setPasteFocusLock(!!event.payload);
    })
  );

  // Auto-close panel when window loses focus (click outside).
  // When we lose focus the other app is already FG — snapshot it for paste.
  // Skip when custom tray-menu took focus (right-click tray while panel open).
  unlisteners.push(
    await appWindow.onFocusChanged(({ payload: focused }) => {
      if (isPasteFocusLock()) return;
      if (!focused && !isWindowMode.value) {
        void (async () => {
          try {
            const tray = await WebviewWindow.getByLabel("tray-menu");
            if (tray && (await tray.isFocused())) return;
          } catch {
            /* ignore */
          }
          void invoke("capture_paste_target").catch((e) =>
            console.debug("[App] capture_paste_target (non-blocking):", e)
          );
          hidePanel();
        })();
      }
    })
  );

  // Listen for open-settings from Rust tray menu
  unlisteners.push(
    await listen("open-settings", () => {
      openSettings();
    })
  );

  // Tray pause/resume syncs Rust → frontend
  unlisteners.push(
    await listen<boolean>("capture-paused", (event) => {
      clipboardStore.setPauseCapture(event.payload);
    })
  );

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
