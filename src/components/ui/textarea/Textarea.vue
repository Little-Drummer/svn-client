<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { useVModel } from "@vueuse/core"
import { cn } from "@/lib/utils"

const props = defineProps<{
  class?: HTMLAttributes["class"]
  defaultValue?: string | number
  modelValue?: string | number
}>()

const emits = defineEmits<{
  (e: "update:modelValue", payload: string | number): void
}>()

const modelValue = useVModel(props, "modelValue", emits, {
  passive: true,
  defaultValue: props.defaultValue,
})
</script>

<template>
  <textarea
    v-model="modelValue"
    :class="cn(
      'ctl-input',
      'flex min-h-[68px] w-full px-2.5 py-2 text-[13px] leading-snug',
      'transition-[box-shadow,background-color] duration-150 ease-out',
      'disabled:cursor-not-allowed',
      props.class,
    )"
  />
</template>
