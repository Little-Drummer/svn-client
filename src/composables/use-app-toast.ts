import { readonly, ref } from 'vue'

export type AppToastType = 'success' | 'error' | 'warning' | 'info'

export interface AppToast {
  id: number
  type: AppToastType
  title: string
  description?: string
}

const toasts = ref<AppToast[]>([])
let nextId = 1

function push(type: AppToastType, title: string, description?: string) {
  const id = nextId++
  toasts.value = [...toasts.value, { id, type, title, description }]
  window.setTimeout(() => dismiss(id), type === 'error' ? 5200 : 3200)
}

export function dismiss(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id)
}

export function useAppToast() {
  return {
    toasts: readonly(toasts),
    dismiss,
    success: (title: string, description?: string) => push('success', title, description),
    error: (title: string, description?: string) => push('error', title, description),
    warning: (title: string, description?: string) => push('warning', title, description),
    info: (title: string, description?: string) => push('info', title, description),
  }
}
