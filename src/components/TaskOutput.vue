<script setup lang="ts">
import { computed } from 'vue'
import { NScrollbar, NTag } from 'naive-ui'

import { useTasksStore, type RunningTask } from '../stores/tasks'

const props = defineProps<{ taskId: string | null }>()
const tasksStore = useTasksStore()

const task = computed<RunningTask | null>(() =>
  props.taskId ? tasksStore.tasks.get(props.taskId) ?? null : null,
)
</script>

<template>
  <div v-if="task" class="task-output">
    <div class="task-header">
      <n-tag
        size="small"
        :type="task.finished ? (task.success ? 'success' : 'error') : 'info'"
      >
        {{ task.finished ? (task.success ? '成功' : '失败') : '运行中' }}
      </n-tag>
      <span class="title">{{ task.title }}</span>
    </div>
    <n-scrollbar class="log mono">
      <div
        v-for="(l, i) in task.lines"
        :key="i"
        :class="['line', l.stream === 'err' ? 'err' : 'out']"
      >
        {{ l.text }}
      </div>
    </n-scrollbar>
  </div>
</template>

<style scoped>
.task-output {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
  border-top: 1px solid rgba(127, 127, 127, 0.2);
}
.task-header {
  display: flex;
  gap: 8px;
  padding: 6px 8px;
  align-items: center;
  font-size: 12px;
}
.title {
  opacity: 0.8;
}
.log {
  flex: 1;
  min-height: 0;
  font-size: 11px;
  background: rgba(127, 127, 127, 0.06);
}
.line {
  padding: 0 8px;
  white-space: pre-wrap;
  word-break: break-all;
}
.line.err {
  color: #d23030;
}
</style>
