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
        >
          <span class="toast-message">{{ item.message }}</span>
          <button
            type="button"
            class="toast-dismiss"
            :aria-label="$t('common.close')"
            @click="dismiss(item.id)"
          >×</button>
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
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  box-shadow: var(--shadow-md);
  font-size: var(--text-base);
  line-height: 1.4;
  text-align: left;
  word-break: break-word;
}

.toast-message {
  flex: 1;
  min-width: 0;
}

.toast-dismiss {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  margin: -2px -2px 0 0;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-tertiary);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  font-family: inherit;
}

.toast-dismiss:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
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
