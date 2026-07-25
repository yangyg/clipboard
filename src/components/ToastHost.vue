<template>
  <Teleport to="body">
    <div class="toast-host">
      <TransitionGroup name="toast">
        <div
          v-for="item in toasts"
          :key="item.id"
          class="toast-item"
          :class="`toast-${item.kind}`"
          :aria-live="item.kind === 'error' ? 'assertive' : 'polite'"
          role="status"
          @click="dismiss(item.id)"
        >
          {{ item.message }}
        </div>
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
  right: 20px;
  top: 20px;
  left: auto;
  transform: none;
  z-index: 1100;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  pointer-events: none;
  max-width: min(420px, calc(100vw - 40px));
}

.toast-item {
  pointer-events: auto;
  padding: 10px 16px;
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  box-shadow: var(--shadow-md);
  font-size: 13px;
  line-height: 1.4;
  cursor: pointer;
  transition: all var(--transition-smooth);
  text-align: left;
  word-break: break-word;
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
