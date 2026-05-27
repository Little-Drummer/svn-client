import { readonly, ref } from 'vue'

export interface ConfirmDialogOptions {
  title: string
  content: string
  confirmText?: string
  cancelText?: string
  destructive?: boolean
}

interface ConfirmDialogState extends Required<ConfirmDialogOptions> {
  open: boolean
}

const state = ref<ConfirmDialogState>({
  open: false,
  title: '',
  content: '',
  confirmText: '确认',
  cancelText: '取消',
  destructive: false,
})

let resolver: ((confirmed: boolean) => void) | null = null

export function confirm(options: ConfirmDialogOptions) {
  state.value = {
    open: true,
    title: options.title,
    content: options.content,
    confirmText: options.confirmText ?? '确认',
    cancelText: options.cancelText ?? '取消',
    destructive: options.destructive ?? false,
  }

  return new Promise<boolean>((resolve) => {
    resolver = resolve
  })
}

export function resolveConfirm(confirmed: boolean) {
  state.value.open = false
  resolver?.(confirmed)
  resolver = null
}

export function useConfirmDialog() {
  return {
    state: readonly(state),
    confirm,
    resolveConfirm,
  }
}
