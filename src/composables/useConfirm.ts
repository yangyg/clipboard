import { ref } from 'vue'
import { i18n } from '../locales'

export interface ConfirmOptions {
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  danger?: boolean
}

export interface ConfirmRequest extends ConfirmOptions {
  confirmText: string
  cancelText: string
  danger: boolean
}

const current = ref<ConfirmRequest | null>(null)
let resolveFn: ((value: boolean) => void) | null = null

interface PendingRequest {
  request: ConfirmRequest
  resolve: (value: boolean) => void
}

// Requests are queued so a second `confirm()` never silently answers the
// first. Previously a pending dialog was auto-resolved with `false`, which both
// threw away the first caller's (never-shown) confirmation AND swapped the
// dialog content under the user's mouse — the user could click 确认 believing
// they were answering the dialog they first saw.
const queue: PendingRequest[] = []

function showNextIfIdle() {
  if (current.value || queue.length === 0) return
  const next = queue.shift()!
  current.value = next.request
  resolveFn = next.resolve
}

function settle(value: boolean) {
  const resolve = resolveFn
  resolveFn = null
  current.value = null
  if (resolve) resolve(value)
  showNextIfIdle()
}

function confirm(options: ConfirmOptions): Promise<boolean> {
  const request: ConfirmRequest = {
    title: options.title,
    message: options.message,
    confirmText: options.confirmText ?? i18n.global.t('common.confirm'),
    cancelText: options.cancelText ?? i18n.global.t('common.cancel'),
    danger: options.danger ?? false,
  }

  return new Promise<boolean>((resolve) => {
    queue.push({ request, resolve })
    showNextIfIdle()
  })
}

export function useConfirm() {
  return { confirm, current, settle }
}
