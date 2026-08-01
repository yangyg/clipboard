<template>
  <Teleport to="body">
    <div class="toast-host">
      <TransitionGroup name="toast">
        <button
          v-for="item in toasts"
          :key="item.id"
          type="button"
          class="toast-item"
          :class="`toast-${item.kind}`"
          :aria-live="item.kind === 'error' ? 'assertive' : 'polite'"
          :aria-label="item.message"
          @click="dismiss(item.id)"
        >
          {{ item.message }}
        </button>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { useToast } from '../composables/useToast'

const { toasts, dismiss } = useToast()
</script>

<style scoped>
.toast-host {
  position: fixed;
  /* Below 38px titlebar; leave room for enter translateY(-8px) */
  right: var(--space-4);
  top: 60px;
  left: auto;
  transform: none;
  z-index: 1100;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--space-2);
  pointer-events: none;
  max-width: min(420px, calc(100vw - 40px));
}

.toast-item {
  pointer-events: auto;
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  box-shadow: var(--shadow-md);
  font-size: var(--text-base);
  line-height: 1.4;
  cursor: pointer;
  transition: border-color var(--transition-smooth);
  text-align: left;
  word-break: break-word;
  font-family: inherit;
}

.toast-item.toast-info {
  border-color: var(--accent);
}

.toast-item.toast-success {
  border-color: var(--success);
}

.toast-item.toast-error {
  border-color: var(--danger);
}

.toast-item.toast-warning {
  border-color: var(--warning);
}

.toast-enter-active,
.toast-leave-active {
  transition:
    opacity var(--transition-smooth),
    transform var(--transition-smooth);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.toast-move {
  transition: transform var(--transition-smooth);
}
</style>
