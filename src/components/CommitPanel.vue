<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NEmpty, NInput, useDialog } from 'naive-ui'

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

const dialog = useDialog()
const tasksStore = useTasksStore()
const toast = useErrorToast()

const message = ref('')
const taskId = ref<string | null>(null)
const submitting = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

const canCommit = computed(
  () => props.checkedPaths.length > 0 && message.value.trim().length > 0 && !submitting.value,
)

async function submit() {
  if (!canCommit.value) return
  const paths = [...props.checkedPaths]
  dialog.warning({
    title: '确认提交',
    content: `将提交 ${paths.length} 个文件到 ${props.workingCopy.url ?? props.workingCopy.path}`,
    positiveText: '提交',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        const id = await api.startCommit(paths, message.value.trim())
        tasksStore.register({
          taskId: id,
          kind: 'commit',
          title: `提交 ${paths.length} 个文件`,
        })
        taskId.value = id
      } catch (e) {
        toast(e, '启动提交失败')
      }
    },
  })
}

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  (finished) => {
    if (finished) {
      const t = taskId.value ? tasksStore.tasks.get(taskId.value) : null
      if (t?.success) {
        message.value = ''
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
        <n-empty description="勾选左侧文件后再提交" size="small" />
      </template>
      <template v-else>
        <div class="hint">已勾选 {{ checkedPaths.length }} 个文件</div>
      </template>
    </div>

    <n-input
      v-model:value="message"
      type="textarea"
      placeholder="提交说明（必填）"
      :autosize="{ minRows: 4, maxRows: 8 }"
      :disabled="submitting"
    />

    <div class="actions">
      <n-button
        type="primary"
        :disabled="!canCommit"
        :loading="submitting"
        @click="submit"
      >
        提交
      </n-button>
    </div>

    <TaskOutput :task-id="taskId" />
  </div>
</template>

<style scoped>
.commit-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  gap: 8px;
  padding: 8px;
}
.summary {
  font-size: 12px;
}
.hint {
  opacity: 0.7;
}
.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
