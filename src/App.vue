<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import MainLayout from './views/MainLayout.vue'
import AppConfirmDialog from './components/AppConfirmDialog.vue'
import AppToasts from './components/AppToasts.vue'
import { useTasksStore } from './stores/tasks'

const isDark = ref(matchMedia('(prefers-color-scheme: dark)').matches)
const media = matchMedia('(prefers-color-scheme: dark)')
media.addEventListener('change', (e) => {
  isDark.value = e.matches
  document.documentElement.classList.toggle('dark', e.matches)
})
document.documentElement.classList.toggle('dark', isDark.value)

const tasks = useTasksStore()

// 禁用浏览器原生右键菜单，让应用更像原生桌面软件；
// 仍保留输入框/文本域内的右键（复制粘贴需要），自定义右键菜单不受影响。
function onGlobalContextMenu(e: MouseEvent) {
  const el = e.target as HTMLElement | null
  if (el?.closest('input, textarea, [contenteditable=""], [contenteditable="true"]')) {
    return
  }
  e.preventDefault()
}

onMounted(() => {
  tasks.ensureListener()
  document.addEventListener('contextmenu', onGlobalContextMenu)
})

onBeforeUnmount(() => {
  document.removeEventListener('contextmenu', onGlobalContextMenu)
})
</script>

<template>
  <MainLayout />
  <AppConfirmDialog />
  <AppToasts />
</template>
