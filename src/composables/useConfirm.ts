import { ref } from 'vue'

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

function settle(value: boolean) {
  if (!resolveFn) return
  const resolve = resolveFn
  resolveFn = null
  current.value = null
  resolve(value)
}

function confirm(options: ConfirmOptions): Promise<boolean> {
  if (resolveFn) {
    settle(false)
  }

  current.value = {
    title: options.title,
    message: options.message,
    confirmText: options.confirmText ?? '确定',
    cancelText: options.cancelText ?? '取消',
    danger: options.danger ?? false,
  }

  return new Promise<boolean>((resolve) => {
    resolveFn = resolve
  })
}

export function useConfirm() {
  return { confirm, current, settle }
}
