<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import TaskOutput from './TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry } from '../types/svn'

const props = defineProps<{
  workingCopy: WorkingCopyEntry
  checkedPaths: string[]
}>()
const emit = defineEmits<{ done: [] }>()

const tasksStore = useTasksStore()
const toast = useErrorToast()

const message = ref('')
const taskId = ref<string | null>(null)
const draftKey = computed(() => `svn-client.commit-draft.${props.workingCopy.id}`)
const submitting = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

watch(
  () => props.workingCopy.id,
  () => {
    message.value = localStorage.getItem(draftKey.value) ?? ''
  },
  { immediate: true },
)

watch(message, (value) => {
  if (value.trim()) localStorage.setItem(draftKey.value, value)
  else localStorage.removeItem(draftKey.value)
})

const canCommit = computed(
  () => props.checkedPaths.length > 0 && message.value.trim().length > 0 && !submitting.value,
)

async function submit() {
  if (!canCommit.value) return
  const paths = [...props.checkedPaths]
  const ok = await confirm({
    title: '确认提交',
    content: `将提交 ${paths.length} 个文件到 ${props.workingCopy.url ?? props.workingCopy.path}`,
    confirmText: '提交',
  })
  if (!ok) return
  try {
    taskId.value = await launchCommit(paths, message.value.trim())
  } catch (e) {
    toast(e, '启动提交失败')
  }
}

// 启动提交并注册任务，retry 指回自身以便失败后用相同参数重跑
async function launchCommit(paths: string[], msg: string): Promise<string> {
  const id = await api.startCommit(paths, msg)
  tasksStore.register({
    taskId: id,
    kind: 'commit',
    title: `提交 ${paths.length} 个文件`,
    command: `svn commit -m ${JSON.stringify(msg)} ${paths.join(' ')}`,
    retry: () => launchCommit(paths, msg),
  })
  return id
}

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  (finished) => {
    if (finished) {
      const t = taskId.value ? tasksStore.tasks.get(taskId.value) : null
      if (t?.success) {
        message.value = ''
        localStorage.removeItem(draftKey.value)
        emit('done')
      }
    }
  },
)
</script>

<template>
  <div class="commit-panel">
    <div class="summary">
      <template v-if="checkedPaths.length === 0">
        <EmptyState description="勾选左侧文件后再提交" />
      </template>
      <template v-else>
        <span class="summary-count">{{ checkedPaths.length }}</span>
        <span class="summary-label">个文件待提交</span>
      </template>
    </div>

    <Textarea
      v-model="message"
      placeholder="提交说明（必填）"
      :disabled="submitting"
      class="commit-message"
    />

    <div class="actions">
      <Button
        :disabled="!canCommit"
        @click="submit"
      >
        {{ submitting ? '提交中' : '提交' }}
      </Button>
    </div>

    <TaskOutput :task-id="taskId" @retried="taskId = $event" />
  </div>
</template>

<style scoped>
.commit-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  gap: 10px;
  padding: 12px;
  background: var(--mat-content);
}
.summary {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: var(--fs-body);
  min-height: 22px;
}
.summary-count {
  font-size: var(--fs-headline);
  font-weight: 600;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.summary-label {
  color: var(--fg-muted);
}
.commit-message {
  min-height: 116px;
  resize: vertical;
}
.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
