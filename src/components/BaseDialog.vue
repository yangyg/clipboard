<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="open"
        class="dialog-overlay"
        @click.self="onOverlayClick"
      >
        <div
          ref="cardRef"
          class="dialog-card"
          :role="role"
          aria-modal="true"
          :aria-labelledby="labelledBy"
          :aria-describedby="describedBy"
          tabindex="-1"
          @keydown="onCardKeydown"
        >
          <slot />
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onUnmounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    role?: string;
    labelledBy?: string;
    describedBy?: string;
    closeOnOverlay?: boolean;
  }>(),
  {
    role: "dialog",
    closeOnOverlay: true,
  },
);

const emit = defineEmits<{
  (e: "close"): void;
}>();

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const cardRef = ref<HTMLElement | null>(null);
let previousFocus: HTMLElement | null = null;

function focusableNodes(): HTMLElement[] {
  if (!cardRef.value) return [];
  return [...cardRef.value.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => el.offsetParent !== null || el === cardRef.value,
  );
}

function onOverlayClick() {
  if (props.closeOnOverlay) emit("close");
}

function onCardKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    emit("close");
    return;
  }
  if (e.key !== "Tab") return;
  const nodes = focusableNodes().filter((el) => el !== cardRef.value);
  if (nodes.length === 0) {
    e.preventDefault();
    return;
  }
  const first = nodes[0];
  const last = nodes[nodes.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

function onWindowKeydown(e: KeyboardEvent) {
  if (!props.open) return;
  if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement | null;
      window.addEventListener("keydown", onWindowKeydown, true);
      await nextTick();
      const nodes = focusableNodes().filter((el) => el !== cardRef.value);
      if (nodes[0]) nodes[0].focus();
      else cardRef.value?.focus();
    } else {
      window.removeEventListener("keydown", onWindowKeydown, true);
      previousFocus?.focus?.();
      previousFocus = null;
    }
  },
);

onUnmounted(() => {
  window.removeEventListener("keydown", onWindowKeydown, true);
});
</script>

<style>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog-card {
  width: 340px;
  max-width: calc(100vw - 2 * var(--space-4));
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg, 14px);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  outline: none;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.dialog-title {
  font-size: var(--text-lg);
  font-weight: 700;
  color: var(--text-primary);
}

.dialog-close {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-tertiary);
  font-size: var(--text-base);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-close:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dialog-body {
  padding: var(--space-4);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

.dialog-footer .btn-cancel,
.dialog-card > .dialog-footer .btn-cancel,
button.btn-cancel {
  height: var(--btn-height-lg);
  padding: 0 var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: var(--text-md);
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border-subtle);
  transition: all var(--transition-fast);
  font-family: inherit;
}

button.btn-cancel:hover {
  background: var(--bg-hover);
}

button.btn-cancel:active,
button.btn-confirm:active {
  transform: scale(0.97);
  filter: brightness(0.94);
}

button.btn-confirm {
  height: var(--btn-height-lg);
  padding: 0 var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: #fff;
  font-size: var(--text-md);
  font-weight: 600;
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
  font-family: inherit;
}

button.btn-confirm:hover {
  background: var(--accent-hover);
}

button.btn-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

button.btn-confirm.danger {
  background: var(--danger);
}

button.btn-confirm.danger:hover {
  filter: brightness(1.08);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity var(--transition-smooth);
}

.modal-enter-active .dialog-card,
.modal-leave-active .dialog-card {
  transition: transform var(--transition-smooth), opacity var(--transition-smooth);
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .dialog-card,
.modal-leave-to .dialog-card {
  transform: translateY(10px) scale(0.98);
  opacity: 0;
}
</style>
