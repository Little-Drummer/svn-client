<script setup lang="ts">
import { CheckCircle2, Info, TriangleAlert, X, XCircle } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { useAppToast, type AppToastType } from '@/composables/use-app-toast'

const toast = useAppToast()

function iconFor(type: AppToastType) {
  switch (type) {
    case 'success':
      return CheckCircle2
    case 'error':
      return XCircle
    case 'warning':
      return TriangleAlert
    default:
      return Info
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="toast-stack">
      <TransitionGroup name="toast">
        <div
          v-for="item in toast.toasts.value"
          :key="item.id"
          :class="['toast-item', item.type]"
        >
          <component :is="iconFor(item.type)" class="toast-icon" />
          <div class="toast-copy">
            <div class="toast-title">{{ item.title }}</div>
            <div v-if="item.description" class="toast-desc">{{ item.description }}</div>
          </div>
          <Button variant="ghost" size="icon-sm" class="toast-close" @click="toast.dismiss(item.id)">
            <X />
          </Button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  z-index: 80;
  right: 18px;
  top: 18px;
  display: flex;
  width: min(380px, calc(100vw - 36px));
  flex-direction: column;
  gap: 10px;
  pointer-events: none;
}
.toast-item {
  pointer-events: auto;
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr) 28px;
  gap: 10px;
  align-items: start;
  padding: 10px 10px 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--panel-bg) 84%, transparent);
  box-shadow: var(--shadow-md);
  backdrop-filter: blur(22px) saturate(145%);
}
.toast-icon {
  width: 17px;
  height: 17px;
  margin-top: 2px;
}
.toast-item.success .toast-icon {
  color: var(--success);
}
.toast-item.error .toast-icon {
  color: var(--destructive);
}
.toast-item.warning .toast-icon {
  color: var(--warning);
}
.toast-item.info .toast-icon {
  color: var(--accent);
}
.toast-copy {
  min-width: 0;
}
.toast-title {
  color: var(--text-strong);
  font-size: 13px;
  font-weight: 600;
}
.toast-desc {
  margin-top: 2px;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.45;
  word-break: break-word;
}
.toast-close {
  width: 26px;
  height: 26px;
  color: var(--text-muted);
}
.toast-enter-active,
.toast-leave-active {
  transition: all 160ms ease;
}
.toast-enter-from,
.toast-leave-to {
  transform: translateY(-6px) scale(0.98);
  opacity: 0;
}
</style>
