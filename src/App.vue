<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NNotificationProvider,
  NLoadingBarProvider,
  zhCN,
  dateZhCN,
  darkTheme,
  type GlobalThemeOverrides,
} from 'naive-ui'

import MainLayout from './views/MainLayout.vue'
import { useTasksStore } from './stores/tasks'

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#1A6BFF',
    primaryColorHover: '#3B82F6',
    primaryColorPressed: '#1957D9',
    primaryColorSuppl: '#3B82F6',
  },
}

const isDark = ref(matchMedia('(prefers-color-scheme: dark)').matches)
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
  isDark.value = e.matches
})

const tasks = useTasksStore()
onMounted(() => {
  tasks.ensureListener()
})
</script>

<template>
  <n-config-provider
    :theme="isDark ? darkTheme : null"
    :theme-overrides="themeOverrides"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <n-loading-bar-provider>
      <n-message-provider>
        <n-dialog-provider>
          <n-notification-provider>
            <MainLayout />
          </n-notification-provider>
        </n-dialog-provider>
      </n-message-provider>
    </n-loading-bar-provider>
  </n-config-provider>
</template>
