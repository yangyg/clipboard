import { ref } from 'vue'

export type ToastKind = 'info' | 'success' | 'error' | 'warning'

export interface ToastItem {
  id: number
  message: string
  kind: ToastKind
}

const toasts = ref<ToastItem[]>([])
let nextId = 1

function dismiss(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id)
}

function toast(message: string, kind: ToastKind = 'info') {
  const id = nextId++
  toasts.value = [...toasts.value, { id, message, kind }]
  window.setTimeout(() => dismiss(id), 1500)
}

export function useToast() {
  return { toast, toasts, dismiss }
}
