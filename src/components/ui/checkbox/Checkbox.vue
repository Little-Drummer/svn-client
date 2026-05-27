<script setup lang="ts">
import type { CheckboxRootEmits, CheckboxRootProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { Check, Minus } from "lucide-vue-next"
import { CheckboxIndicator, CheckboxRoot, useForwardPropsEmits } from "reka-ui"
import { cn } from "@/lib/utils"

const props = defineProps<CheckboxRootProps & { class?: HTMLAttributes["class"] }>()
const emits = defineEmits<CheckboxRootEmits>()

const delegatedProps = reactiveOmit(props, "class")

const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <CheckboxRoot
    v-bind="forwarded"
    :class="cn('mac-checkbox', props.class)"
  >
    <CheckboxIndicator class="mac-checkbox-indicator">
      <Check
        v-if="forwarded.modelValue === true"
        class="mac-checkbox-icon"
      />
      <Minus
        v-else-if="forwarded.modelValue === 'indeterminate'"
        class="mac-checkbox-icon"
      />
      <slot />
    </CheckboxIndicator>
  </CheckboxRoot>
</template>

<style scoped>
.mac-checkbox {
  display: grid;
  place-content: center;
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  border: 0;
  border-radius: 4px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
  cursor: pointer;
  transition: background-color 140ms ease-out, box-shadow 140ms ease-out;
}
.mac-checkbox:hover:not(:disabled) {
  background: color-mix(in srgb, var(--mat-elevated) 90%, var(--fg) 10%);
}
.mac-checkbox:focus-visible {
  outline: none;
  box-shadow:
    var(--stroke-control),
    0 0 0 3px var(--accent-ring);
}
.mac-checkbox[data-state='checked'],
.mac-checkbox[data-state='indeterminate'] {
  background:
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 86%, white 14%) 0%,
      var(--accent) 100%
    );
  box-shadow: var(--stroke-accent);
}
.mac-checkbox:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.mac-checkbox-indicator {
  display: grid;
  place-content: center;
  color: #fff;
  pointer-events: none;
}
.mac-checkbox-icon {
  width: 10px;
  height: 10px;
  stroke-width: 3;
}
</style>
