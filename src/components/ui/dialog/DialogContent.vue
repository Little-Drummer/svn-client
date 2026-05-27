<script setup lang="ts">
import type { DialogContentEmits, DialogContentProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { X } from "lucide-vue-next"
import {
  DialogClose,
  DialogContent,
  DialogOverlay,
  DialogPortal,
  useForwardPropsEmits,
} from "reka-ui"
import { cn } from "@/lib/utils"

const props = defineProps<DialogContentProps & { class?: HTMLAttributes["class"] }>()
const emits = defineEmits<DialogContentEmits>()

const delegatedProps = reactiveOmit(props, "class")

const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <DialogPortal>
    <DialogOverlay class="dialog-overlay" />
    <DialogContent
      v-bind="forwarded"
      :class="cn('dialog-content', props.class)"
    >
      <slot />
      <DialogClose class="dialog-close">
        <X class="w-3.5 h-3.5" />
        <span class="sr-only">Close</span>
      </DialogClose>
    </DialogContent>
  </DialogPortal>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(6px) saturate(120%);
  -webkit-backdrop-filter: blur(6px) saturate(120%);
}
.dialog-overlay[data-state='open'] {
  animation: dialog-fade-in 180ms cubic-bezier(0.32, 0.72, 0, 1);
}
.dialog-overlay[data-state='closed'] {
  animation: dialog-fade-out 140ms ease-out;
}

.dialog-content {
  position: fixed;
  left: 50%;
  top: 50%;
  z-index: 51;
  display: grid;
  width: 100%;
  max-width: 520px;
  transform: translate(-50%, -50%);
  gap: 14px;
  padding: 20px 22px;
  border-radius: var(--radius-window);
  background: var(--mat-popover);
  backdrop-filter: var(--vibrancy-popover);
  -webkit-backdrop-filter: var(--vibrancy-popover);
  box-shadow: var(--shadow-modal);
  color: var(--fg-strong);
  border: 0;
}
.dialog-content[data-state='open'] {
  animation: dialog-pop-in 200ms cubic-bezier(0.32, 0.72, 0, 1);
}
.dialog-content[data-state='closed'] {
  animation: dialog-pop-out 140ms ease-in;
}

.dialog-close {
  position: absolute;
  top: 12px;
  right: 12px;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  border: 0;
  background: transparent;
  color: var(--fg-muted);
  cursor: pointer;
  transition: background-color 140ms ease-out, color 140ms ease-out;
}
.dialog-close:hover {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg-strong);
}
.dialog-close:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px var(--accent-ring);
}

@keyframes dialog-fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
@keyframes dialog-fade-out {
  from { opacity: 1; }
  to { opacity: 0; }
}
@keyframes dialog-pop-in {
  from {
    opacity: 0;
    transform: translate(-50%, -50%) scale(0.96);
  }
  to {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }
}
@keyframes dialog-pop-out {
  from {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }
  to {
    opacity: 0;
    transform: translate(-50%, -50%) scale(0.97);
  }
}
</style>
