<template>
  <div class="tray-shell" ref="root" tabindex="0" @keydown="onKeydown">
    <div class="tray-menu panel-surface" role="menu">
      <template v-for="(item, index) in items" :key="item.id">
        <div v-if="item.separatorBefore" class="sep" role="separator" />
        <button
          type="button"
          class="item"
          :class="{ danger: item.danger, focused: index === focusIndex }"
          role="menuitem"
          @click="onSelect(item.id)"
          @mouseenter="focusIndex = index"
        >
          <span class="icon"><AppIcon :name="item.icon" :size="14" /></span>
          {{ item.label }}
        </button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AppIcon from "./components/icons/AppIcon.vue";
import { buildTrayMenuItems, type TrayMenuItemDef } from "./utils/trayMenuItems";

interface TrayMenuState {
  paused: boolean;
  theme: string;
  enable_blur: boolean;
  panel_opacity: number;
}

const root = ref<HTMLElement | null>(null);
const items = ref<TrayMenuItemDef[]>([]);
const focusIndex = ref(0);
const appWindow = getCurrentWindow();

const unlisteners: UnlistenFn[] = [];

function applyTheme(theme: string) {
  document.body.classList.remove("light-theme", "dark-theme", "oled-theme");
  if (theme === "system") {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.body.classList.add(prefersDark ? "dark-theme" : "light-theme");
  } else if (theme !== "dark") {
    document.body.classList.add(`${theme}-theme`);
  }
}

function applyChrome(state: TrayMenuState) {
  applyTheme(state.theme);
  document.documentElement.style.setProperty(
    "--panel-opacity",
    String(state.panel_opacity / 100),
  );
  document.body.classList.toggle("blur-enabled", state.enable_blur);
}

function applyPaused(paused: boolean) {
  items.value = buildTrayMenuItems(paused);
}

async function refreshState() {
  const state = await invoke<TrayMenuState>("get_tray_menu_state");
  applyChrome(state);
  applyPaused(state.paused);
}

async function focusRoot() {
  await nextTick();
  root.value?.focus();
}

async function hideMenu() {
  await appWindow.hide();
}

async function onSelect(id: TrayMenuItemDef["id"]) {
  try {
    await invoke("tray_menu_action", { action: id });
  } catch (e) {
    console.error("tray_menu_action failed:", e);
  }
}

function onKeydown(e: KeyboardEvent) {
  if (items.value.length === 0) return;
  if (e.key === "Escape") {
    e.preventDefault();
    void hideMenu();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    focusIndex.value = (focusIndex.value + 1) % items.value.length;
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    focusIndex.value = (focusIndex.value - 1 + items.value.length) % items.value.length;
    return;
  }
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    const item = items.value[focusIndex.value];
    if (item) void onSelect(item.id);
  }
}

onMounted(async () => {
  try {
    await refreshState();
  } catch (e) {
    console.error("get_tray_menu_state failed:", e);
    applyPaused(false);
  }
  focusIndex.value = 0;
  await focusRoot();

  unlisteners.push(
    await listen("tray-menu-opened", async () => {
      try {
        await refreshState();
      } catch (e) {
        console.error("get_tray_menu_state failed:", e);
      }
      focusIndex.value = 0;
      await focusRoot();
    }),
  );

  unlisteners.push(
    await listen<boolean>("capture-paused", (event) => {
      applyPaused(!!event.payload);
    }),
  );

  unlisteners.push(
    await listen("settings-updated", async () => {
      try {
        await refreshState();
      } catch (e) {
        console.error("get_tray_menu_state failed:", e);
      }
    }),
  );

  unlisteners.push(
    await appWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) void hideMenu();
    }),
  );
});

onUnmounted(() => {
  for (const unlisten of unlisteners) unlisten();
  unlisteners.length = 0;
});
</script>

<style scoped>
.tray-shell {
  padding: 16px;
  background: transparent;
  outline: none;
}

.tray-menu {
  width: 220px;
  border-radius: var(--radius-lg);
  padding: 8px;
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 10px;
  font-size: var(--text-md);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
  background: transparent;
  border: none;
  font-family: inherit;
  text-align: left;
}

.item:hover,
.item.focused,
.item:focus-visible {
  background: var(--bg-hover);
  color: var(--text-primary);
  outline: none;
}

.item.danger {
  color: var(--danger);
}

.item.danger:hover,
.item.danger.focused,
.item.danger:focus-visible {
  background: var(--danger-soft);
}

.icon {
  width: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--border-subtle);
}
</style>
