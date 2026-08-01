import { ref } from 'vue'

export type ToastKind = 'info' | 'success' | 'error' | 'warning'

export interface ToastItem {
  id: number
  message: string
  kind: ToastKind
}

const toasts = ref<ToastItem[]>([])
let nextId = 1

/** Cap visible toasts so rapid failures cannot stack into an unreadable column. */
const MAX_TOASTS = 5

function dismiss(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id)
}

function toast(message: string, kind: ToastKind = 'info') {
  const existing = toasts.value.find((t) => t.message === message && t.kind === kind)
  if (existing) {
    dismiss(existing.id)
  }
  const id = nextId++
  const next = [...toasts.value, { id, message, kind }]
  toasts.value = next.length > MAX_TOASTS ? next.slice(next.length - MAX_TOASTS) : next
  const duration = kind === 'error' ? 4000 : 1500
  window.setTimeout(() => dismiss(id), duration)
}

export function useToast() {
  return { toast, toasts, dismiss }
}
