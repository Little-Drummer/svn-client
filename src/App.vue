<script setup lang="ts">
import { onMounted, ref } from 'vue'

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
onMounted(() => {
  tasks.ensureListener()
})
</script>

<template>
  <MainLayout />
  <AppConfirmDialog />
  <AppToasts />
</template>
