<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NInput, NTooltip } from 'naive-ui'

import TaskOutput from './TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry }>()
const emit = defineEmits<{ done: [] }>()

const tasksStore = useTasksStore()
const toast = useErrorToast()

const revision = ref('')
const taskId = ref<string | null>(null)
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

async function start() {
  try {
    const id = await api.startUpdate(props.workingCopy.path, revision.value || undefined)
    tasksStore.register({
      taskId: id,
      kind: 'update',
      title: `更新到 ${revision.value || 'HEAD'}`,
    })
    taskId.value = id
  } catch (e) {
    toast(e, '启动更新失败')
  }
}

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  (finished) => {
    if (finished) emit('done')
  },
)
</script>

<template>
  <div class="update-panel">
    <div class="row">
      <span class="label">目标版本</span>
      <n-tooltip>
        <template #trigger>
          <n-input
            v-model:value="revision"
            placeholder="留空 = HEAD"
            size="small"
            :disabled="running"
          />
        </template>
        可填具体 revision、HEAD、{2025-01-01} 等
      </n-tooltip>
    </div>
    <div class="actions">
      <n-button type="primary" :loading="running" @click="start">更新</n-button>
    </div>
    <TaskOutput :task-id="taskId" />
  </div>
</template>

<style scoped>
.update-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 8px;
  padding: 8px;
  min-height: 0;
}
.row {
  display: flex;
  gap: 8px;
  align-items: center;
}
.label {
  font-size: 12px;
  opacity: 0.75;
  min-width: 64px;
}
.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
