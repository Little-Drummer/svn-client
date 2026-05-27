<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { useVModel } from "@vueuse/core"
import { cn } from "@/lib/utils"

const props = defineProps<{
  defaultValue?: string | number
  modelValue?: string | number
  class?: HTMLAttributes["class"]
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
  <input
    v-model="modelValue"
    :class="cn(
      'ctl-input',
      'flex h-7 w-full px-2.5 py-1 text-[13px]',
      'transition-[box-shadow,background-color] duration-150 ease-out',
      'file:border-0 file:bg-transparent file:text-[13px] file:font-medium',
      'disabled:cursor-not-allowed',
      props.class,
    )"
  >
</template>
