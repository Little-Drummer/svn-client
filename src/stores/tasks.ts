import { defineStore } from 'pinia'
import { reactive, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { TaskEvent } from '../types/svn'

// 任务种类（前端用于在面板里区分展示）
export type TaskKind = 'commit' | 'update' | 'checkout'

export interface RunningTask {
  taskId: string
  kind: TaskKind
  title: string
  startedAt: number
  finished: boolean
  success?: boolean
  exitCode?: number | null
  lines: { stream: 'out' | 'err'; text: string }[]
}

export const useTasksStore = defineStore('tasks', () => {
  const tasks = reactive(new Map<string, RunningTask>())
  const activeTaskId = ref<string | null>(null)
  let unlisten: UnlistenFn | null = null

  async function ensureListener() {
    if (unlisten) return
    unlisten = await listen<TaskEvent>('svn-task', (event) => {
      const ev = event.payload
      const t = tasks.get(ev.taskId)
      if (!t) return
      switch (ev.kind) {
        case 'started':
          break
        case 'stdout':
          t.lines.push({ stream: 'out', text: ev.line })
          break
        case 'stderr':
          t.lines.push({ stream: 'err', text: ev.line })
          break
        case 'finished':
          t.finished = true
          t.success = ev.success
          t.exitCode = ev.exitCode ?? null
          break
      }
    })
  }

  function register(task: { taskId: string; kind: TaskKind; title: string }) {
    tasks.set(task.taskId, {
      taskId: task.taskId,
      kind: task.kind,
      title: task.title,
      startedAt: Date.now(),
      finished: false,
      lines: [],
    })
    activeTaskId.value = task.taskId
  }

  function clear(taskId: string) {
    tasks.delete(taskId)
    if (activeTaskId.value === taskId) activeTaskId.value = null
  }

  return { tasks, activeTaskId, ensureListener, register, clear }
})
