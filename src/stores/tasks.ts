import { defineStore } from 'pinia'
import { computed, reactive, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import type { TaskEvent } from '../types/svn'
import { api, describeError } from '../api/svn'

// 任务种类（前端用于在面板里区分展示）
export type TaskKind = 'commit' | 'update' | 'checkout' | 'merge' | 'package'

// 单个任务输出最多保留的行数，超出丢弃最旧的，避免长任务撑爆内存/DOM
const MAX_LINES = 5000

export interface RunningTask {
  taskId: string
  kind: TaskKind
  workingCopyId?: string // 关联工作副本，用于任务完成后精准刷新对应状态。
  title: string
  command?: string // 等价命令行（敏感参数已打码），供用户核对实际执行的命令
  startedAt: number
  finished: boolean
  success?: boolean
  exitCode?: number | null
  canceled?: boolean // 由用户主动终止
  canceling?: boolean // 已发出终止请求、等待进程退出
  lines: { stream: 'out' | 'err'; text: string }[]
}

export const useTasksStore = defineStore('tasks', () => {
  const tasks = reactive(new Map<string, RunningTask>())
  const activeTaskId = ref<string | null>(null)
  // 显式发布任务完成信号，避免依赖 Map 内部对象变更的深度监听。
  const completedTask = ref<{
    taskId: string
    kind: TaskKind
    workingCopyId?: string
    success: boolean
    version: number
  } | null>(null)
  let completedTaskVersion = 0
  // 自增信号：从没有独立输出面板的入口（如工作副本右键更新）发起任务时，请求弹开任务中心
  const centerOpenRequest = ref(0)
  // 持有 listen 的 Promise 而非结果做防重：多个组件挂载时并发调用，
  // 若等结果返回才置位，两次调用都会通过检查、注册出两个监听器，输出就会逐行重复
  let listenerPromise: Promise<UnlistenFn> | null = null

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

  // 后端在 invoke 返回前就开始 emit，极快任务的事件可能先于 register 到达；
  // 按 taskId 暂存这些早到事件，register 时回放，否则 finished 丢失会让任务永远显示运行中
  const earlyEvents = new Map<string, TaskEvent[]>()
  const EARLY_TASKS_MAX = 16

  function applyEvent(ev: TaskEvent) {
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
        t.canceled = ev.canceled ?? false
        t.canceling = false
        completedTask.value = {
          taskId: t.taskId,
          kind: t.kind,
          workingCopyId: t.workingCopyId,
          success: ev.success,
          version: ++completedTaskVersion,
        }
        break
    }
  }

  async function ensureListener() {
    if (listenerPromise) {
      await listenerPromise
      return
    }
    listenerPromise = listen<TaskEvent>('svn-task', (event) => {
      const ev = event.payload
      if (!tasks.has(ev.taskId)) {
        let buf = earlyEvents.get(ev.taskId)
        if (!buf) {
          // 永远等不到 register 的任务（如旧实例残留）按先进先出淘汰，防止暂存无限增长
          if (earlyEvents.size >= EARLY_TASKS_MAX) {
            const oldest = earlyEvents.keys().next().value
            if (oldest !== undefined) {
              earlyEvents.delete(oldest)
            }
          }
          buf = []
          earlyEvents.set(ev.taskId, buf)
        }
        buf.push(ev)
        return
      }
      applyEvent(ev)
    })
    await listenerPromise
  }

  function register(task: {
    taskId: string
    kind: TaskKind
    workingCopyId?: string
    title: string
    command?: string
    retry?: () => Promise<string>
  }) {
    tasks.set(task.taskId, {
      taskId: task.taskId,
      kind: task.kind,
      workingCopyId: task.workingCopyId,
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

    // 回放注册前已到达的事件（含可能的 finished）
    const buffered = earlyEvents.get(task.taskId)
    if (buffered) {
      earlyEvents.delete(task.taskId)
      for (const ev of buffered) {
        applyEvent(ev)
      }
    }
  }

  // 重试失败的任务：调用注册时记录的回调，返回新任务 id（无回调则返回 null）
  async function retry(taskId: string): Promise<string | null> {
    const fn = retries.get(taskId)
    if (!fn) return null
    return await fn()
  }

  // 收到 kill 信号后，正常应很快收到后端 finished 事件；这是该兜底时长（毫秒）
  const CANCEL_FALLBACK_MS = 4000

  // 把任务强制收尾为「已终止」，用于后端无对应进程、或 finished 事件丢失时让 UI 脱困
  function finalizeCanceled(taskId: string, note: string) {
    const t = tasks.get(taskId)
    if (!t || t.finished) {
      return
    }
    flushPending() // 先落地已有输出，再追加终止说明
    t.lines.push({ stream: 'err', text: note })
    t.finished = true
    t.success = false
    t.canceled = true
    t.canceling = false
  }

  // 终止运行中的任务：请求后端 kill 子进程。无论后端结果如何，都确保 UI 最终脱离「运行中」。
  async function cancel(taskId: string) {
    const t = tasks.get(taskId)
    if (!t || t.finished || t.canceling) {
      return
    }
    t.canceling = true

    let running: boolean
    try {
      running = await api.cancelTask(taskId)
    } catch (err) {
      // 调用后端失败（如命令不存在/旧实例）：仍强制结束显示，避免 UI 永久卡死
      finalizeCanceled(taskId, `[终止] 调用后端失败：${describeError(err)}`)
      return
    }

    if (!running) {
      // 后端注册表里没有该运行中任务（已结束或属旧实例），直接按已终止收尾
      finalizeCanceled(taskId, '[终止] 后端未找到对应运行中进程，已结束显示')
      return
    }

    // 已发出 kill，等后端 finished(canceled=true)。兜底：若数秒内仍未收到，强制收尾防卡死。
    window.setTimeout(() => {
      const cur = tasks.get(taskId)
      if (cur && !cur.finished) {
        finalizeCanceled(
          taskId,
          '[终止] 已发送终止信号，但未在数秒内收到进程退出确认，已强制结束显示',
        )
      }
    }, CANCEL_FALLBACK_MS)
  }

  function openCenter() {
    centerOpenRequest.value += 1
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

  return {
    tasks,
    activeTaskId,
    completedTask,
    centerOpenRequest,
    runningCount,
    ensureListener,
    register,
    retry,
    cancel,
    openCenter,
    clear,
  }
})
