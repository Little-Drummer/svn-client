<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Maximize2, Minimize2, X } from 'lucide-vue-next'

import TaskOutput from './TaskOutput.vue'
import { useTasksStore, type RunningTask } from '../stores/tasks'

const tasksStore = useTasksStore()
const open = ref(false)
// 输出多（如大更新）时切换成大尺寸面板，会话内记住选择
const expanded = ref(false)
const selectedId = ref<string | null>(null)
const panelRef = ref<HTMLElement | null>(null)

// 新任务在最上面，便于刚发起的更新一眼可见
const taskList = computed<RunningTask[]>(() =>
  Array.from(tasksStore.tasks.values()).reverse(),
)
const hasTasks = computed(() => taskList.value.length > 0)

const failedCount = computed(
  () => taskList.value.filter((t) => t.finished && t.success === false && !t.canceled).length,
)

// 触发器状态：运行中优先，其次失败，否则全部完成
const triggerState = computed<'running' | 'failed' | 'done'>(() => {
  if (tasksStore.runningCount > 0) return 'running'
  if (failedCount.value > 0) return 'failed'
  return 'done'
})

const triggerLabel = computed(() => {
  if (triggerState.value === 'running') return `${tasksStore.runningCount} 个任务运行中`
  if (triggerState.value === 'failed') return `${failedCount.value} 个任务失败`
  return '任务已完成'
})

function rowState(t: RunningTask): 'running' | 'failed' | 'canceled' | 'done' {
  if (!t.finished) return 'running'
  if (t.canceled) return 'canceled'
  return t.success === false ? 'failed' : 'done'
}

function toggle() {
  open.value = !open.value
}

function clearTask(id: string) {
  const wasSelected = selectedId.value === id
  tasksStore.clear(id)
  if (wasSelected) {
    selectedId.value = taskList.value[0]?.taskId ?? null
  }
  if (!hasTasks.value) open.value = false
}

function clearFinished() {
  for (const t of taskList.value) {
    if (t.finished) tasksStore.clear(t.taskId)
  }
  if (selectedId.value && !tasksStore.tasks.get(selectedId.value)) {
    selectedId.value = taskList.value[0]?.taskId ?? null
  }
  if (!hasTasks.value) open.value = false
}

// 打开时默认选中当前活动任务；选中项失效时回退到最新一条
watch(open, (v) => {
  if (v) {
    const active = tasksStore.activeTaskId
    const valid = selectedId.value && tasksStore.tasks.get(selectedId.value)
    selectedId.value = valid ? selectedId.value : active ?? taskList.value[0]?.taskId ?? null
    document.addEventListener('pointerdown', onPointerDown, true)
    document.addEventListener('keydown', onKeydown, true)
  } else {
    teardown()
  }
})

// 任务全部被清空时自动收起浮层
watch(hasTasks, (v) => {
  if (!v) open.value = false
})

// 外部入口（如右键更新）请求弹开：选中最新任务并展开
watch(
  () => tasksStore.centerOpenRequest,
  () => {
    selectedId.value = tasksStore.activeTaskId
    open.value = true
  },
)

function onPointerDown(e: MouseEvent) {
  const target = e.target as Node
  if (panelRef.value?.contains(target)) return
  // 点触发器本身交给它的 click 处理，避免这里先关掉又被重新打开
  if ((target as HTMLElement).closest?.('.tc-trigger')) return
  open.value = false
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') open.value = false
}
function teardown() {
  document.removeEventListener('pointerdown', onPointerDown, true)
  document.removeEventListener('keydown', onKeydown, true)
}
onBeforeUnmount(teardown)
</script>

<template>
  <button
    v-if="hasTasks"
    type="button"
    class="tc-trigger statusbar-text"
    :class="[`is-${triggerState}`, { 'is-open': open }]"
    @click="toggle"
  >
    <span class="tc-dot" />
    {{ triggerLabel }}
  </button>

  <Teleport to="body">
    <Transition name="tc-pop">
      <div v-if="open && hasTasks" ref="panelRef" class="tc-panel" :class="{ 'is-expanded': expanded }">
        <header class="tc-head">
          <span class="tc-head-title">任务</span>
          <span class="tc-spacer" />
          <button
            v-if="taskList.some((t) => t.finished)"
            type="button"
            class="tc-clear-all"
            @click="clearFinished"
          >
            清除已完成
          </button>
          <button
            type="button"
            class="tc-close"
            :title="expanded ? '还原' : '放大'"
            @click="expanded = !expanded"
          >
            <Minimize2 v-if="expanded" class="tc-close-icon" />
            <Maximize2 v-else class="tc-close-icon" />
          </button>
          <button type="button" class="tc-close" title="关闭" @click="open = false">
            <X class="tc-close-icon" />
          </button>
        </header>

        <div v-if="taskList.length > 1" class="tc-list">
          <button
            v-for="t in taskList"
            :key="t.taskId"
            type="button"
            class="tc-row"
            :class="[`is-${rowState(t)}`, { active: t.taskId === selectedId }]"
            @click="selectedId = t.taskId"
          >
            <span class="tc-row-dot" />
            <span class="tc-row-title">{{ t.title }}</span>
            <span
              class="tc-row-clear"
              title="移除"
              @click.stop="clearTask(t.taskId)"
            >
              <X class="tc-row-clear-icon" />
            </span>
          </button>
        </div>

        <div class="tc-output">
          <TaskOutput :task-id="selectedId" @retried="selectedId = $event" />
        </div>

        <footer v-if="taskList.length === 1" class="tc-foot">
          <button
            v-if="taskList[0].finished"
            type="button"
            class="tc-clear-all"
            @click="clearTask(taskList[0].taskId)"
          >
            清除
          </button>
        </footer>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ===== 状态栏触发器 ===== */
.tc-trigger {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 20px;
  padding: 0 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--fg-muted);
  font: inherit;
  cursor: default;
  transition: background-color 120ms ease-out, color 120ms ease-out;
}
.tc-trigger:hover,
.tc-trigger.is-open {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg);
}
.tc-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex: none;
}
.tc-trigger.is-running .tc-dot {
  background: var(--accent);
  animation: tc-pulse 1.2s ease-in-out infinite;
}
.tc-trigger.is-failed {
  color: var(--danger);
}
.tc-trigger.is-failed .tc-dot {
  background: var(--danger);
}
.tc-trigger.is-done .tc-dot {
  background: var(--success, #30a46c);
}
@keyframes tc-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.8); }
}

/* ===== 浮层面板（右下角，悬于状态栏之上）===== */
.tc-panel {
  position: fixed;
  right: 12px;
  bottom: 38px;
  z-index: 900;
  width: 440px;
  max-width: calc(100vw - 24px);
  height: 360px;
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 12px;
  background: var(--mat-elevated, rgba(248, 248, 248, 0.96));
  backdrop-filter: blur(24px) saturate(180%);
  -webkit-backdrop-filter: blur(24px) saturate(180%);
  border: var(--hairline, 0.5px) solid var(--stroke, rgba(0, 0, 0, 0.12));
  box-shadow:
    0 0 0 0.5px rgba(0, 0, 0, 0.05),
    0 12px 40px rgba(0, 0, 0, 0.24);
  transition:
    width 180ms cubic-bezier(0.32, 0.72, 0, 1),
    height 180ms cubic-bezier(0.32, 0.72, 0, 1);
}
.tc-panel.is-expanded {
  width: min(900px, calc(100vw - 24px));
  height: min(720px, calc(100vh - 80px));
}
.tc-panel.is-expanded .tc-list {
  max-height: 200px;
}
.tc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 10px 9px 14px;
  flex: none;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.tc-head-title {
  font-size: var(--fs-callout);
  font-weight: 600;
  color: var(--fg-strong);
}
.tc-spacer {
  flex: 1;
}
.tc-clear-all {
  border: none;
  background: transparent;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  padding: 2px 6px;
  border-radius: 5px;
  cursor: default;
}
.tc-clear-all:hover {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg);
}
.tc-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--fg-muted);
  cursor: default;
}
.tc-close:hover {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg);
}
.tc-close-icon {
  width: 14px;
  height: 14px;
}

/* ===== 多任务列表 ===== */
.tc-list {
  flex: none;
  max-height: 132px;
  overflow: auto;
  padding: 4px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.tc-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 8px;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--fg);
  font: inherit;
  text-align: left;
  cursor: default;
}
.tc-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.tc-row.active {
  background: var(--accent);
  color: #fff;
}
.tc-row-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
}
.tc-row.is-running .tc-row-dot {
  background: var(--accent);
  animation: tc-pulse 1.2s ease-in-out infinite;
}
.tc-row.active.is-running .tc-row-dot {
  background: #fff;
}
.tc-row.is-failed .tc-row-dot {
  background: var(--danger);
}
.tc-row.is-done .tc-row-dot {
  background: var(--success, #30a46c);
}
.tc-row.is-canceled .tc-row-dot {
  background: var(--fg-subtle);
}
.tc-row-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-callout);
}
.tc-row-clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 5px;
  flex: none;
  opacity: 0;
  color: var(--fg-muted);
}
.tc-row:hover .tc-row-clear {
  opacity: 1;
}
.tc-row.active .tc-row-clear {
  color: rgba(255, 255, 255, 0.8);
}
.tc-row-clear:hover {
  background: color-mix(in srgb, var(--fg) 12%, transparent);
}
.tc-row.active .tc-row-clear:hover {
  background: rgba(255, 255, 255, 0.2);
}
.tc-row-clear-icon {
  width: 12px;
  height: 12px;
}

/* ===== 输出区 ===== */
.tc-output {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.tc-output :deep(.task-output) {
  border-top: none;
}
.tc-foot {
  flex: none;
  display: flex;
  justify-content: flex-end;
  padding: 6px 10px;
  border-top: var(--hairline) solid var(--stroke-soft);
}

/* ===== 弹出动画 ===== */
.tc-pop-enter-active {
  transition: opacity 120ms ease-out, transform 120ms cubic-bezier(0.32, 0.72, 0, 1);
}
.tc-pop-leave-active {
  transition: opacity 90ms ease-in, transform 90ms ease-in;
}
.tc-pop-enter-from,
.tc-pop-leave-to {
  opacity: 0;
  transform: translateY(6px) scale(0.98);
}
@media (prefers-reduced-motion: reduce) {
  .tc-pop-enter-active,
  .tc-pop-leave-active {
    transition: opacity 90ms ease;
  }
  .tc-pop-enter-from,
  .tc-pop-leave-to {
    transform: none;
  }
}
</style>
