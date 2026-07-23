<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuRef"
      class="context-menu"
      role="menu"
      :style="{ left: pos.x + 'px', top: pos.y + 'px', width: width + 'px' }"
      @click.stop
      @keydown="onKeydown"
    >
      <template v-for="(item, index) in items" :key="item.id">
        <div v-if="item.separatorBefore" class="ctx-sep" role="separator" />
        <button
          type="button"
          class="ctx-item"
          :class="{ danger: item.danger, focused: index === focusIndex }"
          role="menuitem"
          :tabindex="index === focusIndex ? 0 : -1"
          @click="select(item.id)"
          @mouseenter="focusIndex = index"
        >
          <span v-if="item.icon" class="ctx-icon">
            <AppIcon :name="item.icon" :size="14" />
          </span>
          {{ item.label }}
          <span v-if="item.shortcut" class="ctx-shortcut">{{ item.shortcut }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import AppIcon, { type AppIconName } from "./icons/AppIcon.vue";

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: AppIconName;
  shortcut?: string;
  danger?: boolean;
  separatorBefore?: boolean;
}

const props = withDefaults(
  defineProps<{
    visible: boolean;
    x: number;
    y: number;
    items: ContextMenuItem[];
    width?: number;
  }>(),
  { width: 190 },
);

const emit = defineEmits<{
  (e: "close"): void;
  (e: "select", id: string): void;
}>();

const menuRef = ref<HTMLElement | null>(null);
const focusIndex = ref(0);
const pos = reactive({ x: 0, y: 0 });

function clamp() {
  const el = menuRef.value;
  if (!el) {
    pos.x = props.x;
    pos.y = props.y;
    return;
  }
  const rect = el.getBoundingClientRect();
  const pad = 8;
  pos.x = Math.max(pad, Math.min(props.x, window.innerWidth - rect.width - pad));
  pos.y = Math.max(pad, Math.min(props.y, window.innerHeight - rect.height - pad));
}

function select(id: string) {
  emit("select", id);
  emit("close");
}

function onKeydown(e: KeyboardEvent) {
  if (!props.visible || props.items.length === 0) return;
  if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    focusIndex.value = (focusIndex.value + 1) % props.items.length;
    focusItem();
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    focusIndex.value = (focusIndex.value - 1 + props.items.length) % props.items.length;
    focusItem();
    return;
  }
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    const item = props.items[focusIndex.value];
    if (item) select(item.id);
  }
}

function focusItem() {
  nextTick(() => {
    const buttons = menuRef.value?.querySelectorAll<HTMLButtonElement>(".ctx-item");
    buttons?.[focusIndex.value]?.focus();
  });
}

function onGlobalPointer(e: MouseEvent) {
  if (!props.visible) return;
  if (menuRef.value?.contains(e.target as Node)) return;
  emit("close");
}

watch(
  () => props.visible,
  async (v) => {
    if (v) {
      focusIndex.value = 0;
      pos.x = props.x;
      pos.y = props.y;
      await nextTick();
      clamp();
      focusItem();
    }
  },
);

watch(
  () => [props.x, props.y, props.items.length] as const,
  async () => {
    if (!props.visible) return;
    await nextTick();
    clamp();
  },
);

onMounted(() => {
  window.addEventListener("mousedown", onGlobalPointer, true);
  window.addEventListener("keydown", onKeydown, true);
});

onUnmounted(() => {
  window.removeEventListener("mousedown", onGlobalPointer, true);
  window.removeEventListener("keydown", onKeydown, true);
});
</script>

<style scoped>
.context-menu {
  position: fixed;
  background: var(--bg-surface);
  border: 1px solid var(--border-default, var(--border-subtle));
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  padding: 6px;
  z-index: 1100;
}

.ctx-item {
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

.ctx-item:hover,
.ctx-item.focused,
.ctx-item:focus-visible {
  background: var(--bg-hover);
  color: var(--text-primary);
  outline: none;
}

.ctx-item.danger {
  color: var(--danger);
}

.ctx-item.danger:hover,
.ctx-item.danger.focused,
.ctx-item.danger:focus-visible {
  background: var(--danger-soft);
}

.ctx-icon {
  width: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.ctx-shortcut {
  margin-left: auto;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  color: var(--text-tertiary);
}

.ctx-sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--border-subtle);
}
</style>
