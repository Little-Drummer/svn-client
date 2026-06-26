<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { Ban, RotateCw, Terminal, Wrench } from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { api, describeError } from '../api/svn'
import { useTasksStore, type RunningTask } from '../stores/tasks'

const props = defineProps<{ taskId: string | null }>()
const emit = defineEmits<{ (e: 'retried', taskId: string): void }>()
const tasksStore = useTasksStore()

const task = computed<RunningTask | null>(() =>
  props.taskId ? tasksStore.tasks.get(props.taskId) ?? null : null,
)

// 运行超过该秒数后提示「可能已卡住」并把终止按钮变醒目
const SLOW_THRESHOLD_S = 60

const showCommand = ref(false)
const retrying = ref(false)
const canceling = ref(false)

const failed = computed(
  () => task.value?.finished === true && task.value?.success === false && !task.value?.canceled,
)

// 失败或被终止的任务都允许重试（终止常用于卡死后重来）
const canRetry = computed(() => failed.value || task.value?.canceled === true)

// 每秒一次的时钟，仅在有任务运行时启动，用于驱动耗时计时
const now = ref(Date.now())
let timer: number | null = null
function startTimer() {
  if (timer != null) return
  timer = window.setInterval(() => {
    now.value = Date.now()
  }, 1000)
}
function stopTimer() {
  if (timer != null) {
    clearInterval(timer)
    timer = null
  }
}
onBeforeUnmount(stopTimer)

const running = computed(() => task.value != null && !task.value.finished)

// 运行中保持计时器，结束即停，避免空转
watch(
  running,
  (v) => {
    if (v) {
      now.value = Date.now()
      startTimer()
    } else {
      stopTimer()
    }
  },
  { immediate: true },
)

// 计时器在任务结束时停止，now 冻结在最后一拍，因此结束后展示的约等于总耗时
const elapsedS = computed(() => {
  const t = task.value
  if (!t) return 0
  return Math.max(0, Math.floor((now.value - t.startedAt) / 1000))
})

function fmtDuration(total: number): string {
  const m = Math.floor(total / 60)
  const s = total % 60
  return `${m}:${String(s).padStart(2, '0')}`
}

const elapsedLabel = computed(() => fmtDuration(elapsedS.value))

const isSlow = computed(() => running.value && elapsedS.value >= SLOW_THRESHOLD_S)

async function onCancel() {
  if (!props.taskId || canceling.value) return
  canceling.value = true
  try {
    await tasksStore.cancel(props.taskId)
  } catch (err) {
    console.error(err)
  } finally {
    canceling.value = false
  }
}

async function onRetry() {
  if (!props.taskId || retrying.value) return
  retrying.value = true
  try {
    const newId = await tasksStore.retry(props.taskId)
    if (newId) emit('retried', newId)
  } finally {
    retrying.value = false
  }
}

// ===== 终止后的锁恢复：svn cleanup + 重试 =====

const cleaningUp = ref(false)
const cleanupError = ref<string | null>(null)

// 需要 cleanup 的工作副本路径：
// - 失败任务：从 E155004 输出里解析 Working copy '<path>' locked
// - 被终止的任务：kill 几乎必然留锁，直接用 Updating '<path>' 行里的根目录
const cleanupPath = computed<string | null>(() => {
  const t = task.value
  if (!t || !t.finished) return null
  if (t.success === false && !t.canceled) {
    for (let i = t.lines.length - 1; i >= 0; i--) {
      const m = t.lines[i].text.match(/Working copy '(.+)' locked/)
      if (m) return m[1]
    }
    return null
  }
  if (t.canceled) {
    const prefix = pathPrefix.value
    return prefix ? prefix.replace(/\/$/, '') : null
  }
  return null
})

async function onCleanupRetry() {
  const path = cleanupPath.value
  if (!path || cleaningUp.value || retrying.value) return
  cleaningUp.value = true
  cleanupError.value = null
  try {
    await api.cleanup(path)
    await onRetry()
  } catch (err) {
    cleanupError.value = describeError(err)
  } finally {
    cleaningUp.value = false
  }
}

const lineCount = computed(() => task.value?.lines.length ?? 0)

// ===== svn 输出行解析：动作行转成彩色标签 + 路径，元信息行弱化 =====

type LineView =
  | { kind: 'action'; label: string; cls: string; path: string }
  | { kind: 'meta'; cls: string; text: string }
  | { kind: 'plain' | 'err'; text: string }

// update/checkout/merge 输出的动作字母 → 标签与配色
const ACTION_META: Record<string, { label: string; cls: string }> = {
  A: { label: '新增', cls: 'act-add' },
  U: { label: '更新', cls: 'act-upd' },
  D: { label: '删除', cls: 'act-del' },
  C: { label: '冲突', cls: 'act-conf' },
  G: { label: '合并', cls: 'act-merge' },
  R: { label: '替换', cls: 'act-rep' },
  E: { label: '已有', cls: 'act-exist' },
  '!': { label: '缺失', cls: 'act-conf' },
}

// commit 输出用英文动词而非字母，归并到同一套标签
const COMMIT_VERBS: Record<string, { label: string; cls: string }> = {
  Sending: { label: '发送', cls: 'act-upd' },
  Adding: { label: '新增', cls: 'act-add' },
  Deleting: { label: '删除', cls: 'act-del' },
  Replacing: { label: '替换', cls: 'act-rep' },
}

// 过程性说明行（弱化显示）；revision 结尾行单独加重，作为任务的结果行
const META_RE =
  /^(Updating '|Checking out |Restored '|Fetching external|External |--- |Summary of conflicts|Transmitting file data|Skipped |Resolved conflicted state)/
const REV_RE = /^(At|Updated to|Checked out|Committed) revision \d+/

// svn 以绝对路径为目标时动作行也打印绝对路径，从 Updating/Checking out 行提取根目录用于裁剪
const pathPrefix = computed(() => {
  const t = task.value
  if (!t) return null
  const limit = Math.min(t.lines.length, 8)
  for (let i = 0; i < limit; i++) {
    const m = t.lines[i].text.match(/^(?:Updating|Checking out) '(.+)':/)
    if (m) {
      return m[1].endsWith('/') ? m[1] : `${m[1]}/`
    }
  }
  return null
})

function stripPrefix(path: string): string {
  const prefix = pathPrefix.value
  if (prefix && path.startsWith(prefix)) {
    return path.slice(prefix.length)
  }
  return path
}

function parseLine(text: string, stream: 'out' | 'err'): LineView {
  if (stream === 'err') {
    return { kind: 'err', text }
  }
  // 首列或属性列（带一个前导空格）的动作字母，后面至少两个空格再接路径
  const action = text.match(/^ ?([ADUCGER!]) {2,}(\S.*)$/)
  if (action) {
    const meta = ACTION_META[action[1]]
    if (meta) {
      return { kind: 'action', label: meta.label, cls: meta.cls, path: stripPrefix(action[2]) }
    }
  }
  const verb = text.match(/^(Sending|Adding|Deleting|Replacing)\s+(.+)$/)
  if (verb) {
    const meta = COMMIT_VERBS[verb[1]]
    return { kind: 'action', label: meta.label, cls: meta.cls, path: stripPrefix(verb[2]) }
  }
  if (REV_RE.test(text)) {
    return { kind: 'meta', cls: 'rev', text }
  }
  if (META_RE.test(text)) {
    return { kind: 'meta', cls: '', text }
  }
  return { kind: 'plain', text }
}

function lineView(index: number): LineView {
  const line = task.value?.lines[index]
  if (!line) {
    return { kind: 'plain', text: '' }
  }
  return parseLine(line.text, line.stream)
}

const badgeVariant = computed<'success' | 'destructive' | 'secondary'>(() => {
  const t = task.value
  if (!t) return 'secondary'
  if (!t.finished) return 'secondary'
  if (t.canceled) return 'secondary'
  return t.success ? 'success' : 'destructive'
})

const badgeText = computed(() => {
  const t = task.value
  if (!t) return ''
  if (!t.finished) return t.canceling ? '终止中…' : '运行中'
  if (t.canceled) return '已终止'
  return t.success ? '成功' : '失败'
})

const logScrollRef = ref<HTMLElement | null>(null)

const logVirtualizer = useVirtualizer(
  computed(() => {
    const el = logScrollRef.value
    return {
      count: lineCount.value,
      getScrollElement: () => el,
      estimateSize: () => 21,
      overscan: 20,
    }
  }),
)

// 可见行预先解析好，模板里不用对同一行重复调 lineView
const virtualRows = computed(() =>
  logVirtualizer.value.getVirtualItems().map((v) => ({
    index: v.index,
    start: v.start,
    view: lineView(v.index),
  })),
)

// 自动滚到底：仅当用户当前就在底部时才跟随，避免打断用户回看历史
const stickToBottom = ref(true)

function onScroll() {
  const el = logScrollRef.value
  if (!el) return
  const distance = el.scrollHeight - el.scrollTop - el.clientHeight
  stickToBottom.value = distance < 24
}

watch(lineCount, async () => {
  if (!stickToBottom.value) return
  await nextTick()
  const count = lineCount.value
  if (count > 0) {
    logVirtualizer.value.scrollToIndex(count - 1, { align: 'end' })
  }
})

// 行高随内容（折行）变化，交给虚拟器实测，避免固定行高导致长行互相重叠
function measureRow(el: unknown) {
  if (el instanceof HTMLElement) {
    logVirtualizer.value.measureElement(el)
  }
}

// 切任务时回到顶部并重置跟随与 cleanup 提示
watch(
  () => props.taskId,
  () => {
    stickToBottom.value = true
    cleanupError.value = null
  },
)
</script>

<template>
  <div v-if="task" class="task-output">
    <div class="task-header">
      <Badge :variant="badgeVariant" :class="['task-badge', !task.finished && 'is-running']">
        <span v-if="!task.finished" class="run-dot" />
        {{ badgeText }}
      </Badge>
      <span class="title">{{ task.title }}</span>
      <span v-if="running" class="elapsed mono" :class="{ slow: isSlow }">
        {{ elapsedLabel }}
      </span>
      <span class="spacer" />
      <span v-if="isSlow && !task.canceling" class="slow-hint">运行较久，可能已卡住</span>
      <Button
        v-if="running"
        size="xs"
        variant="ghost"
        class="header-btn cancel-btn"
        :class="{ urgent: isSlow }"
        :disabled="task.canceling || canceling"
        title="终止任务"
        @click="onCancel"
      >
        <Ban class="icon-xs" />
        {{ task.canceling ? '终止中' : '终止' }}
      </Button>
      <Button
        v-if="task.command"
        size="xs"
        variant="ghost"
        class="header-btn"
        :class="{ active: showCommand }"
        title="显示等价命令"
        @click="showCommand = !showCommand"
      >
        <Terminal class="icon-xs" />
      </Button>
      <Button
        v-if="canRetry && cleanupPath"
        size="xs"
        variant="ghost"
        class="header-btn"
        :disabled="cleaningUp || retrying"
        title="执行 svn cleanup 解除工作副本锁后重新发起任务"
        @click="onCleanupRetry"
      >
        <Wrench class="icon-xs" :class="{ spin: cleaningUp }" />
        {{ cleaningUp ? '清理中' : '清理并重试' }}
      </Button>
      <Button
        v-if="canRetry"
        size="xs"
        variant="ghost"
        class="header-btn"
        :disabled="retrying || cleaningUp"
        @click="onRetry"
      >
        <RotateCw class="icon-xs" :class="{ spin: retrying }" />
        重试
      </Button>
    </div>
    <div v-if="cleanupError" class="cleanup-error">cleanup 失败：{{ cleanupError }}</div>
    <div v-if="showCommand && task.command" class="command-bar mono">
      <span class="command-prompt">$</span>
      {{ task.command }}
    </div>
    <div ref="logScrollRef" class="log mono virtual-scroll" @scroll.passive="onScroll">
      <div
        class="virtual-stage"
        :style="{ height: `${logVirtualizer.getTotalSize()}px` }"
      >
        <div
          v-for="vRow in virtualRows"
          :key="vRow.index"
          :ref="measureRow"
          :data-index="vRow.index"
          class="virtual-slot"
          :style="{ transform: `translateY(${vRow.start}px)` }"
        >
          <div v-if="vRow.view.kind === 'action'" class="line action">
            <span class="act-chip" :class="vRow.view.cls">{{ vRow.view.label }}</span>
            <span
              class="act-path"
              :class="{ conflict: vRow.view.cls === 'act-conf' }"
              :title="vRow.view.path"
            >
              {{ vRow.view.path }}
            </span>
          </div>
          <div v-else-if="vRow.view.kind === 'meta'" class="line meta" :class="vRow.view.cls">
            {{ vRow.view.text }}
          </div>
          <div v-else :class="['line', vRow.view.kind === 'err' ? 'err' : 'out']">
            {{ vRow.view.text }}
          </div>
        </div>
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
  border-top: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-content);
}
.task-header {
  display: flex;
  gap: 8px;
  padding: 7px 12px;
  align-items: center;
  font-size: var(--fs-callout);
  background: var(--mat-toolbar);
  border-bottom: var(--hairline) solid var(--stroke-soft);
  min-height: 32px;
}
.title {
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.header-btn {
  gap: 4px;
  color: var(--fg-muted);
}
.header-btn.active {
  color: var(--accent);
}
.elapsed {
  flex: none;
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
  font-variant-numeric: tabular-nums;
}
.elapsed.slow {
  color: var(--warning, #d97706);
}
.slow-hint {
  flex: none;
  font-size: var(--fs-caption);
  color: var(--warning, #d97706);
  white-space: nowrap;
}
.cancel-btn {
  color: var(--fg-muted);
}
.cancel-btn:hover {
  color: var(--danger);
}
.cancel-btn.urgent {
  color: var(--danger);
}
.icon-xs {
  width: 13px;
  height: 13px;
}
.icon-xs.spin {
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
.command-bar {
  display: flex;
  gap: 6px;
  padding: 6px 12px;
  font-size: var(--fs-mono);
  color: var(--fg-muted);
  background: color-mix(in srgb, var(--mat-toolbar) 90%, var(--fg) 6%);
  border-bottom: var(--hairline) solid var(--stroke-soft);
  word-break: break-all;
  white-space: pre-wrap;
}
.command-prompt {
  color: var(--fg-subtle);
  user-select: none;
}
.cleanup-error {
  padding: 5px 12px;
  font-size: var(--fs-caption);
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 8%, transparent);
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.log {
  flex: 1;
  min-height: 0;
  overflow: auto;
  font-size: var(--fs-mono);
  background: color-mix(in srgb, var(--mat-content) 92%, var(--fg) 8%);
}
.dark .log {
  background: color-mix(in srgb, var(--mat-content) 80%, black 20%);
}
.task-badge {
  height: 18px;
  gap: 4px;
}
.task-badge.is-running {
  color: var(--accent);
  background: var(--accent-soft);
}
.run-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
  animation: pulse-dot 1.2s ease-in-out infinite;
}
@keyframes pulse-dot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.85); }
}
.line {
  padding: 1px 12px;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--fg-muted);
  line-height: 1.55;
}
.line.err {
  color: var(--danger);
}

/* ===== 动作行：彩色标签 + 相对路径 ===== */
.line.action {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}
.act-chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  height: 16px;
  padding: 0 5px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 600;
  flex: none;
  font-family: var(--font-ui, system-ui);
}
.act-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--fg);
}
.act-path.conflict {
  color: var(--danger);
  font-weight: 600;
}
.act-add {
  color: var(--success, #30a46c);
  background: color-mix(in srgb, var(--success, #30a46c) 14%, transparent);
}
.act-upd {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
.act-del {
  color: var(--warning, #d97706);
  background: color-mix(in srgb, var(--warning, #d97706) 14%, transparent);
}
.act-conf {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
}
.act-merge {
  color: #0d9488;
  background: color-mix(in srgb, #0d9488 14%, transparent);
}
.act-rep {
  color: #8b5cf6;
  background: color-mix(in srgb, #8b5cf6 14%, transparent);
}
.act-exist {
  color: var(--fg-subtle);
  background: color-mix(in srgb, var(--fg) 8%, transparent);
}

/* ===== 元信息行：过程说明弱化，revision 结果行加重 ===== */
.line.meta {
  color: var(--fg-subtle);
}
.line.meta.rev {
  color: var(--fg);
  font-weight: 500;
}

/* ============ 虚拟滚动布局 ============ */
.virtual-scroll {
  position: relative;
}
.virtual-stage {
  position: relative;
  width: 100%;
}
.virtual-slot {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
}
/* 行高由内容决定并被虚拟器实测，不再固定高度 */
</style>
