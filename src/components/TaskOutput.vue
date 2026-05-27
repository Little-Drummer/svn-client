<script setup lang="ts">
import { computed } from 'vue'

import { Badge } from '@/components/ui/badge'
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
      <Badge
        :variant="task.finished ? (task.success ? 'secondary' : 'destructive') : 'outline'"
        :class="['task-badge', task.finished ? (task.success ? 'success' : 'error') : 'running']"
      >
        {{ task.finished ? (task.success ? '成功' : '失败') : '运行中' }}
      </Badge>
      <span class="title">{{ task.title }}</span>
    </div>
    <div class="log mono">
      <div
        v-for="(l, i) in task.lines"
        :key="i"
        :class="['line', l.stream === 'err' ? 'err' : 'out']"
      >
        {{ l.text }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.task-output {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
  overflow: hidden;
  border-top: 1px solid var(--border);
  background: var(--panel-bg);
}
.task-header {
  display: flex;
  gap: 8px;
  padding: 7px 10px;
  align-items: center;
  font-size: 12px;
  background: var(--panel-bg-subtle);
  border-bottom: 1px solid var(--border-subtle);
}
.title {
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.log {
  flex: 1;
  min-height: 0;
  overflow: auto;
  font-size: 11px;
  background: var(--panel-bg-muted);
}
.task-badge {
  height: 20px;
}
.task-badge.success {
  color: var(--success);
}
.task-badge.running {
  color: var(--accent);
}
.line {
  padding: 1px 10px;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--text-muted);
}
.line.err {
  color: #dc2626;
}
</style>
