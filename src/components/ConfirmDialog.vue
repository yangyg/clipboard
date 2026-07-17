<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="current"
        class="dialog-overlay"
        @click.self="settle(false)"
      >
        <div
          class="dialog-card"
          role="alertdialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          :aria-describedby="messageId"
        >
          <div class="dialog-header">
            <span :id="titleId" class="dialog-title">{{ current.title }}</span>
          </div>
          <div class="dialog-body">
            <p :id="messageId" class="dialog-message">{{ current.message }}</p>
          </div>
          <div class="dialog-footer">
            <button class="btn-cancel" type="button" @click="settle(false)">
              {{ current.cancelText }}
            </button>
            <button
              class="btn-confirm"
              :class="{ danger: current.danger }"
              type="button"
              @click="settle(true)"
            >
              {{ current.confirmText }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useConfirm } from '../composables/useConfirm'

const { current, settle } = useConfirm()

const titleId = 'confirm-dialog-title'
const messageId = 'confirm-dialog-message'

function onKeydown(e: KeyboardEvent) {
  if (!current.value) return
  if (e.key === 'Escape') {
    e.preventDefault()
    settle(false)
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
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
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg, 14px);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.dialog-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
}

.dialog-body {
  padding: 16px;
}

.dialog-message {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn-cancel {
  height: 32px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border-subtle);
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-cancel:hover {
  background: var(--bg-hover);
}

.btn-confirm {
  height: 32px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: #fff;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-confirm:hover {
  background: var(--accent-light, #6b85fa);
}

.btn-confirm.danger {
  background: var(--danger);
}

.btn-confirm.danger:hover {
  filter: brightness(1.08);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}

.modal-enter-active .dialog-card,
.modal-leave-active .dialog-card {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .dialog-card,
.modal-leave-to .dialog-card {
  transform: scale(0.95);
  opacity: 0;
}
</style>
