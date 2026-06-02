import { defineStore } from 'pinia'
import { computed, reactive, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import type { TaskEvent } from '../types/svn'

// 任务种类（前端用于在面板里区分展示）
export type TaskKind = 'commit' | 'update' | 'checkout'

// 单个任务输出最多保留的行数，超出丢弃最旧的，避免长任务撑爆内存/DOM
const MAX_LINES = 5000

export interface RunningTask {
  taskId: string
  kind: TaskKind
  title: string
  command?: string // 等价命令行（敏感参数已打码），供用户核对实际执行的命令
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

  // 重试回调与行缓冲都不放进响应式对象，避免无谓的依赖追踪
  const retries = new Map<string, () => Promise<string>>()
  const pending = new Map<string, { stream: 'out' | 'err'; text: string }[]>()
  let flushScheduled = false

  // 按帧合并行写入：高频 stdout 不再每行触发一次响应式更新
  function scheduleFlush() {
    if (flushScheduled) {
      return
    }
    flushScheduled = true
    requestAnimationFrame(() => {
      flushScheduled = false
      flushPending()
    })
  }

  function flushPending() {
    for (const [id, buf] of pending) {
      const t = tasks.get(id)
      if (t && buf.length > 0) {
        t.lines.push(...buf)
        if (t.lines.length > MAX_LINES) {
          t.lines.splice(0, t.lines.length - MAX_LINES)
        }
      }
    }
    pending.clear()
  }

  function bufferLine(taskId: string, stream: 'out' | 'err', text: string) {
    let buf = pending.get(taskId)
    if (!buf) {
      buf = []
      pending.set(taskId, buf)
    }
    buf.push({ stream, text })
    scheduleFlush()
  }

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
          bufferLine(ev.taskId, 'out', ev.line)
          break
        case 'stderr':
          bufferLine(ev.taskId, 'err', ev.line)
          break
        case 'finished':
          flushPending() // 结束前把缓冲里的尾行落地，避免漏掉最后输出
          t.finished = true
          t.success = ev.success
          t.exitCode = ev.exitCode ?? null
          break
      }
    })
  }

  function register(task: {
    taskId: string
    kind: TaskKind
    title: string
    command?: string
    retry?: () => Promise<string>
  }) {
    tasks.set(task.taskId, {
      taskId: task.taskId,
      kind: task.kind,
      title: task.title,
      command: task.command,
      startedAt: Date.now(),
      finished: false,
      lines: [],
    })
    if (task.retry) {
      retries.set(task.taskId, task.retry)
    }
    activeTaskId.value = task.taskId
  }

  // 重试失败的任务：调用注册时记录的回调，返回新任务 id（无回调则返回 null）
  async function retry(taskId: string): Promise<string | null> {
    const fn = retries.get(taskId)
    if (!fn) return null
    return await fn()
  }

  function clear(taskId: string) {
    tasks.delete(taskId)
    retries.delete(taskId)
    pending.delete(taskId)
    if (activeTaskId.value === taskId) activeTaskId.value = null
  }

  // 运行中任务数，用于 Dock 角标与状态栏提示
  const runningCount = computed(() => {
    let n = 0
    for (const t of tasks.values()) {
      if (!t.finished) n += 1
    }
    return n
  })

  // 运行中任务数变化时更新 Dock 角标（macOS），为 0 时清除
  watch(runningCount, (count) => {
    getCurrentWindow()
      .setBadgeCount(count > 0 ? count : undefined)
      .catch(() => {
        // 非 macOS 或不支持时忽略
      })
  })

  return { tasks, activeTaskId, runningCount, ensureListener, register, retry, clear }
})
