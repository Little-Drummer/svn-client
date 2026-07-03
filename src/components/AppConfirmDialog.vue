<script setup lang="ts">
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useConfirmDialog } from '@/composables/use-confirm-dialog'

const dialog = useConfirmDialog()
</script>

<template>
  <Dialog :open="dialog.state.value.open" @update:open="(open) => !open && dialog.resolveConfirm(null)">
    <DialogContent class="confirm-dialog">
      <DialogHeader>
        <DialogTitle>{{ dialog.state.value.title }}</DialogTitle>
        <DialogDescription class="confirm-copy">
          {{ dialog.state.value.content }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="confirm-actions">
        <Button variant="outline" @click="dialog.resolveConfirm(false)">
          {{ dialog.state.value.cancelText }}
        </Button>
        <Button
          :variant="dialog.state.value.destructive ? 'destructive' : 'default'"
          @click="dialog.resolveConfirm(true)"
        >
          {{ dialog.state.value.confirmText }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style>
/* 确认框会叠加在其他弹窗（搁置、冲突解决等）之上打开，必须始终处于最顶层；
   普通弹窗 overlay/content 为 50/51，这里整体抬高一档 */
.dialog-content.confirm-dialog {
  z-index: 61;
}
.dialog-overlay:has(+ .confirm-dialog) {
  z-index: 60;
}
</style>

<style scoped>
.confirm-dialog {
  width: min(520px, calc(100vw - 32px));
  max-width: min(520px, calc(100vw - 32px));
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel-bg) 92%, transparent);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(26px) saturate(150%);
}
.confirm-copy {
  max-height: 50vh;
  overflow-y: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  color: var(--text);
}
.confirm-actions {
  gap: 8px;
  justify-content: flex-end;
}
</style>
