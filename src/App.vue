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
    primaryColor: '#0F8B8D',
    primaryColorHover: '#0C7779',
    primaryColorPressed: '#096163',
    primaryColorSuppl: '#4FD1C5',
    borderRadius: '6px',
    borderRadiusSmall: '5px',
    fontFamily:
      "system-ui, 'PingFang SC', 'Microsoft YaHei', 'Segoe UI', Roboto, sans-serif",
  },
  Tag: {
    borderRadius: '5px',
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
