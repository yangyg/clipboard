<template>
  <div
    class="tray-shell"
    ref="root"
    tabindex="0"
    role="menu"
    :aria-activedescendant="activeDescendantId"
    @keydown="onKeydown"
  >
    <div class="tray-menu" ref="menuEl">
      <template v-for="(item, index) in items" :key="item.id">
        <div v-if="item.separatorBefore" class="sep" role="separator" />
        <button
          :id="`tray-item-${item.id}`"
          type="button"
          class="item"
          :class="{ danger: item.danger, focused: index === focusIndex }"
          role="menuitem"
          :tabindex="index === focusIndex ? 0 : -1"
          @click="onSelect(item.id)"
          @mouseenter="focusIndex = index"
        >
          <span class="icon"><AppIcon :name="item.icon" :size="14" /></span>
          <span class="label">{{ item.label }}</span>
        </button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AppIcon from "./components/icons/AppIcon.vue";
import { buildTrayMenuItems, type TrayMenuItemDef } from "./utils/trayMenuItems";
import { setLocale, resolveLocale } from "./locales";
import { useTrayTheme, type TrayMenuState } from "./composables/useTrayTheme";

const MENU_WIDTH = 176;

const root = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
const items = ref<TrayMenuItemDef[]>([]);
const focusIndex = ref(0);
const appWindow = getCurrentWindow();
const activeDescendantId = computed(() => {
  const item = items.value[focusIndex.value];
  return item ? `tray-item-${item.id}` : undefined;
});

const unlisteners: UnlistenFn[] = [];

function focusActiveItem() {
  void nextTick(() => {
    const item = items.value[focusIndex.value];
    if (!item) return;
    document.getElementById(`tray-item-${item.id}`)?.focus();
  });
}

watch(focusIndex, () => focusActiveItem());

const { applyChrome } = useTrayTheme();

function applyPaused(paused: boolean) {
  items.value = buildTrayMenuItems(paused);
}

async function refreshState() {
  const state = await invoke<TrayMenuState>("get_tray_menu_state");
  applyChrome(state);
  setLocale(resolveLocale(state.language));
  applyPaused(state.paused);
}

async function fitWindowToContent() {
  await nextTick();
  const el = menuEl.value ?? root.value;
  if (!el) return;
  const height = Math.ceil(el.getBoundingClientRect().height);
  if (height <= 0) return;
  try {
    await appWindow.setSize(new LogicalSize(MENU_WIDTH, height));
  } catch (e) {
    console.error("tray-menu setSize failed:", e);
  }
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
    focusActiveItem();
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    focusIndex.value = (focusIndex.value - 1 + items.value.length) % items.value.length;
    focusActiveItem();
    return;
  }
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    const item = items.value[focusIndex.value];
    if (item) void onSelect(item.id);
  }
}

async function onOpened() {
  try {
    await refreshState();
  } catch (e) {
    console.error("get_tray_menu_state failed:", e);
  }
  focusIndex.value = 0;
  await fitWindowToContent();
  await focusRoot();
  focusActiveItem();
}

onMounted(async () => {
  try {
    await refreshState();
  } catch (e) {
    console.error("get_tray_menu_state failed:", e);
    applyPaused(false);
  }
  focusIndex.value = 0;
  await fitWindowToContent();
  await focusRoot();
  focusActiveItem();

  unlisteners.push(await listen("tray-menu-opened", () => void onOpened()));

  unlisteners.push(
    await listen<boolean>("capture-paused", async (event) => {
      applyPaused(!!event.payload);
      await fitWindowToContent();
    }),
  );

  unlisteners.push(
    await listen("settings-updated", async () => {
      try {
        await refreshState();
      } catch (e) {
        console.error("get_tray_menu_state failed:", e);
      }
      await fitWindowToContent();
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
/* Size to content — do not stretch to window height (avoids bottom blank) */
.tray-shell {
  box-sizing: border-box;
  width: 100%;
  height: auto;
  margin: 0;
  padding: 0;
  background: transparent;
  outline: none;
  overflow: hidden;
  border-radius: var(--radius-lg);
}

/* Own surface styles — avoid .panel-surface (--panel-radius clash) */
.tray-menu {
  box-sizing: border-box;
  width: 100%;
  height: auto;
  padding: 6px;
  overflow: hidden;
  background: color-mix(
    in srgb,
    var(--bg-surface) calc(var(--panel-opacity, 0.94) * 100%),
    transparent
  );
  border: 1px solid var(--border-default, var(--border-subtle));
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
}

:global(body.blur-enabled) .tray-menu {
  /* Native DWM acrylic backdrop (set via set_window_backdrop); keep the surface
     translucent so the blurred desktop shows through instead of being covered. */
  background: color-mix(in srgb, var(--bg-surface) calc(var(--panel-blur-opacity, 0.55) * 100%), transparent);
}

.item {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 7px 8px;
  font-size: var(--text-md);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 14px;
  width: 14px;
  height: 14px;
  color: inherit;
}

.label {
  min-width: 0;
  line-height: 1.25;
}

.sep {
  height: 1px;
  margin: 4px 4px;
  background: var(--border-subtle);
}
</style>
