<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import TaskOutput from './TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry; checkedPaths?: string[] }>()
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
    const target =
      props.checkedPaths && props.checkedPaths.length === 1
        ? props.checkedPaths[0]
        : props.workingCopy.path
    taskId.value = await launchUpdate(target, revision.value || undefined)
  } catch (e) {
    toast(e, '启动更新失败')
  }
}

async function launchUpdate(target: string, rev: string | undefined): Promise<string> {
  const id = await api.startUpdate(target, rev)
  const single = props.checkedPaths?.length === 1
  tasksStore.register({
    taskId: id,
    kind: 'update',
    workingCopyId: props.workingCopy.id,
    title: `更新 ${single ? '选中文件' : '整个工作副本'} 到 ${rev || 'HEAD'}`,
    command: `svn update${rev ? ` -r ${rev}` : ''} ${target}`,
    retry: () => launchUpdate(target, rev),
  })
  return id
}

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  (finished) => {
    const task = taskId.value ? tasksStore.tasks.get(taskId.value) : null
    if (finished && task?.success) emit('done')
  },
)
</script>

<template>
  <div class="update-panel">
    <div class="row">
      <span class="label">范围</span>
      <span class="target mono" :title="checkedPaths?.length === 1 ? checkedPaths[0] : workingCopy.path">
        {{ checkedPaths?.length === 1 ? checkedPaths[0] : workingCopy.path }}
      </span>
    </div>
    <div class="row">
      <span class="label">目标版本</span>
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger as-child>
            <Input
              v-model="revision"
              placeholder="留空 = HEAD"
              :disabled="running"
            />
          </TooltipTrigger>
          <TooltipContent>可填具体 revision、HEAD、{2025-01-01} 等</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
    <div class="actions">
      <Button :disabled="running" @click="start">{{ running ? '更新中' : '更新' }}</Button>
    </div>
    <TaskOutput :task-id="taskId" @retried="taskId = $event" />
  </div>
</template>

<style scoped>
.update-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 10px;
  padding: 12px;
  min-height: 0;
  background: var(--mat-content);
}
.row {
  display: flex;
  gap: 10px;
  align-items: center;
}
.label {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  min-width: 64px;
  font-weight: 500;
}
.actions {
  display: flex;
  justify-content: flex-end;
}
.target {
  min-width: 0;
  flex: 1;
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
