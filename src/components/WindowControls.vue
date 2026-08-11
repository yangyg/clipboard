<template>
  <div class="window-controls">
    <button
      type="button"
      class="win-btn"
      :class="{ active: isAlwaysOnTop }"
      :aria-label="isAlwaysOnTop ? $t('common.unpinWindow') : $t('common.pinWindow')"
      :aria-pressed="isAlwaysOnTop"
      :title="isAlwaysOnTop ? $t('common.unpinWindow') : $t('common.pinWindow')"
      @click.stop="toggleAlwaysOnTop"
    >
      <AppIcon name="pin" :size="14" :fill="isAlwaysOnTop ? 'currentColor' : 'none'" />
    </button>
    <button type="button" class="win-btn" :aria-label="$t('common.minimize')" :title="$t('common.minimize')" @click.stop="minimize">
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M1 5h8" stroke="currentColor" stroke-width="1.2" fill="none" />
      </svg>
    </button>
    <button
      type="button"
      class="win-btn"
      :aria-label="maximized ? $t('common.restoreWindow') : $t('common.maximize')"
      :title="maximized ? $t('common.restoreWindow') : $t('common.maximize')"
      @click.stop="toggleMaximize"
    >
      <!-- Restore: two overlapping squares -->
      <svg v-if="maximized" width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path
          d="M3 1.5h5.5v5.5M1.5 3.5H7v5.5H1.5z"
          stroke="currentColor"
          stroke-width="1.1"
          fill="none"
        />
      </svg>
      <!-- Maximize: single square -->
      <svg v-else width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <rect x="1.5" y="1.5" width="7" height="7" stroke="currentColor" stroke-width="1.2" fill="none" />
      </svg>
    </button>
    <button type="button" class="win-btn win-btn-close" :aria-label="$t('common.close')" :title="$t('common.close')" @click.stop="close">
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.2" fill="none" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AppIcon from "./icons/AppIcon.vue";
import { useSettingsStore } from "../stores/settings";

const appWindow = getCurrentWindow();
const maximized = ref(false);
let unlistenResize: (() => void) | undefined;

const settingsStore = useSettingsStore();
const isAlwaysOnTop = computed(() => settingsStore.settings.always_on_top);

function toggleAlwaysOnTop() {
  settingsStore.updateSetting("always_on_top", !settingsStore.settings.always_on_top);
}

async function refreshMaximized() {
  try {
    maximized.value = await appWindow.isMaximized();
  } catch {
    /* ignore */
  }
}

async function minimize() {
  await appWindow.minimize();
}

async function toggleMaximize() {
  await appWindow.toggleMaximize();
  await refreshMaximized();
}

async function close() {
  // CloseRequested hides to tray
  await appWindow.close();
}

onMounted(async () => {
  await refreshMaximized();
  unlistenResize = await appWindow.onResized(() => {
    refreshMaximized();
  });
});

onUnmounted(() => {
  unlistenResize?.();
});
</script>

<style scoped>
.window-controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  margin-left: 2px;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}

.win-btn {
  width: 46px;
  height: 100%;
  min-height: 38px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 0;
  padding: 0;
  transition: background var(--transition-fast), color var(--transition-fast);
}

.win-btn:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.win-btn :deep(.app-icon) {
  transition: transform var(--transition-fast);
}

.win-btn.active {
  color: var(--accent);
}

.win-btn.active :deep(.app-icon) {
  transform: rotate(45deg);
}

.win-btn.active:hover {
  background: var(--accent-softer);
  color: var(--accent-text);
}

.win-btn-close:hover {
  background: var(--win-close-hover);
  color: #fff;
}
</style>
