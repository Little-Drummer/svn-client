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
  <Dialog :open="dialog.state.value.open" @update:open="(open) => !open && dialog.resolveConfirm(false)">
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

<style scoped>
.confirm-dialog {
  max-width: min(520px, calc(100vw - 32px));
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel-bg) 92%, transparent);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(26px) saturate(150%);
}
.confirm-copy {
  white-space: pre-wrap;
  color: var(--text);
}
.confirm-actions {
  gap: 8px;
}
</style>
