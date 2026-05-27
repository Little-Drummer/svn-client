<script setup lang="ts">
import type { SwitchRootEmits, SwitchRootProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import {
  SwitchRoot,
  SwitchThumb,
  useForwardPropsEmits,
} from "reka-ui"
import { cn } from "@/lib/utils"

const props = defineProps<SwitchRootProps & { class?: HTMLAttributes["class"] }>()

const emits = defineEmits<SwitchRootEmits>()

const delegatedProps = reactiveOmit(props, "class")

const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <SwitchRoot
    v-bind="forwarded"
    :class="cn('mac-switch', props.class)"
  >
    <SwitchThumb class="mac-switch-thumb">
      <slot name="thumb" />
    </SwitchThumb>
  </SwitchRoot>
</template>

<style scoped>
.mac-switch {
  position: relative;
  display: inline-flex;
  align-items: center;
  width: 28px;
  height: 16px;
  border: 0;
  padding: 1px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--fg) 18%, transparent);
  box-shadow: inset 0 0 0 0.5px var(--stroke-soft);
  cursor: pointer;
  transition: background-color 180ms cubic-bezier(0.32, 0.72, 0, 1);
  flex-shrink: 0;
}
.mac-switch:focus-visible {
  outline: none;
  box-shadow:
    inset 0 0 0 0.5px var(--stroke-soft),
    0 0 0 3px var(--accent-ring);
}
.mac-switch[data-state='checked'] {
  background: var(--accent);
  box-shadow: inset 0 0 0 0.5px color-mix(in srgb, var(--accent) 50%, black 50%);
}
.mac-switch:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.mac-switch-thumb {
  display: block;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow:
    0 0 0 0.5px rgba(0, 0, 0, 0.18),
    0 1.5px 3px rgba(0, 0, 0, 0.22),
    0 0.5px 0 rgba(255, 255, 255, 0.4) inset;
  pointer-events: none;
  transform: translateX(0);
  transition: transform 200ms cubic-bezier(0.32, 0.72, 0, 1);
}
.mac-switch[data-state='checked'] .mac-switch-thumb {
  transform: translateX(12px);
}
</style>
