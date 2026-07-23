<template>
  <div class="app-root">
    <!-- Floating mode: keep panel mounted (v-show) to avoid full remount cost -->
    <template v-if="!isWindowMode">
      <FloatingPanel v-show="panelVisible && !settingsVisible" @close="hidePanel" @openSettings="openSettings" />
      <SettingsWindow v-if="settingsVisible" @close="closeSettings" />
    </template>
    <!-- Window mode: panel always visible, settings replaces panel -->
    <template v-else>
      <WindowApp v-if="panelVisible && !settingsVisible" @openSettings="openSettings" />
      <SettingsWindow v-if="settingsVisible" @close="closeSettings" />
    </template>
    <ToastHost />
    <ConfirmDialog />
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
import { useClipboardStore } from "./stores/clipboard";
import { useSettingsStore } from "./stores/settings";
import { storeToRefs } from "pinia";
import { isPasteFocusLock, setPasteFocusLock } from "./composables/pasteFocusLock";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { settings } = storeToRefs(settingsStore);

const panelVisible = ref(false);
const settingsVisible = ref(false);
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
  await reloadPanelIfNeeded(false);
  await appWindow.show();
  await appWindow.setFocus();
}

async function hidePanel() {
  if (isWindowMode.value) {
    panelVisible.value = true;
    settingsVisible.value = false;
    await appWindow.show();
    return;
  }
  panelVisible.value = false;
  settingsVisible.value = false;
  await appWindow.hide();
}

function closeSettings() {
  settingsVisible.value = false;
  panelVisible.value = true;
}

async function openSettings() {
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
  await applyAppMode();

  // showPanel() loads records once (avoid a duplicate get_records on cold start)
  await clipboardStore.loadTags();
  lastPanelReloadAt = 0; // force first load
  await showPanel();

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
          void invoke("capture_paste_target").catch(() => {});
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
});
</script>

<style scoped>
.app-root {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}
</style>
