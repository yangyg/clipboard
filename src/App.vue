<template>
  <div class="app-root">
    <!-- Floating mode: swap between panel and settings -->
    <template v-if="!isWindowMode">
      <FloatingPanel v-if="panelVisible" @close="hidePanel" @openSettings="openSettings" />
      <SettingsWindow v-else-if="settingsVisible" @close="closeSettings" />
    </template>
    <!-- Window mode: panel always visible, settings replaces panel -->
    <template v-else>
      <WindowApp v-if="panelVisible && !settingsVisible" @openSettings="openSettings" />
      <SettingsWindow v-if="settingsVisible" @close="closeSettings" />
    </template>
    <TrayMenu
      @open-panel="showPanel"
      @open-settings="openSettings"
    />
    <ToastHost />
    <ConfirmDialog />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import FloatingPanel from "./components/FloatingPanel.vue";
import WindowApp from "./components/WindowApp.vue";
import SettingsWindow from "./components/SettingsWindow.vue";
import TrayMenu from "./components/TrayMenu.vue";
import ToastHost from "./components/ToastHost.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import { useClipboardStore } from "./stores/clipboard";
import { useSettingsStore } from "./stores/settings";
import { storeToRefs } from "pinia";

const clipboardStore = useClipboardStore();
const settingsStore = useSettingsStore();
const { settings } = storeToRefs(settingsStore);

const panelVisible = ref(false);
const settingsVisible = ref(false);

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

async function showPanel() {
  panelVisible.value = true;
  settingsVisible.value = false;
  await clipboardStore.loadRecords();
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
    panelVisible.value = false;
    settingsVisible.value = true;
  }
  await appWindow.show();
  await appWindow.setFocus();
}

onMounted(async () => {
  // Load settings
  await settingsStore.loadSettings();
  await applyAppMode();

  // Load initial records
  await clipboardStore.loadRecords();
  await clipboardStore.loadTags();

  // Show the panel when the window becomes visible
  showPanel();

  // Listen for new clipboard records from Rust backend
  await listen<any>("clipboard-changed", (event) => {
    if (!clipboardStore.pauseCapture) {
      clipboardStore.onNewRecord(event.payload);
    }
  });

  // Listen for toggle-panel from Rust (Rust shows/hides window, we sync panelVisible)
  await listen<boolean>("toggle-panel", (event) => {
    if (event.payload) {
      if (!panelVisible.value) {
        showPanel();
      }
    } else {
      if (panelVisible.value) {
        hidePanel();
      }
    }
  });

  // Auto-close panel when window loses focus (click outside)
  appWindow.onFocusChanged(({ payload: focused }) => {
    if (!focused && !isWindowMode.value) {
      hidePanel();
    }
  });

  // Listen for open-settings from Rust tray menu
  await listen("open-settings", () => {
    openSettings();
  });

  // Tray pause/resume syncs Rust → frontend
  await listen<boolean>("capture-paused", (event) => {
    clipboardStore.setPauseCapture(event.payload);
  });

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
