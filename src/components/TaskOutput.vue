<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { RotateCw, Terminal } from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useTasksStore, type RunningTask } from '../stores/tasks'

const props = defineProps<{ taskId: string | null }>()
const emit = defineEmits<{ (e: 'retried', taskId: string): void }>()
const tasksStore = useTasksStore()

const showCommand = ref(false)
const retrying = ref(false)

const failed = computed(() => task.value?.finished === true && task.value?.success === false)

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

const task = computed<RunningTask | null>(() =>
  props.taskId ? tasksStore.tasks.get(props.taskId) ?? null : null,
)

const lineCount = computed(() => task.value?.lines.length ?? 0)

const badgeVariant = computed<'success' | 'destructive' | 'secondary'>(() => {
  if (!task.value) return 'secondary'
  if (!task.value.finished) return 'secondary'
  return task.value.success ? 'success' : 'destructive'
})

const badgeText = computed(() => {
  if (!task.value) return ''
  if (!task.value.finished) return '运行中'
  return task.value.success ? '成功' : '失败'
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

// 切任务时回到顶部并重置跟随
watch(
  () => props.taskId,
  () => {
    stickToBottom.value = true
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
      <span class="spacer" />
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
        v-if="failed"
        size="xs"
        variant="ghost"
        class="header-btn"
        :disabled="retrying"
        @click="onRetry"
      >
        <RotateCw class="icon-xs" :class="{ spin: retrying }" />
        重试
      </Button>
    </div>
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
          v-for="vRow in logVirtualizer.getVirtualItems()"
          :key="vRow.index"
          class="virtual-slot"
          :style="{ transform: `translateY(${vRow.start}px)`, height: `${vRow.size}px` }"
        >
          <div
            :class="['line', task.lines[vRow.index].stream === 'err' ? 'err' : 'out']"
          >
            {{ task.lines[vRow.index].text }}
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
.virtual-slot > .line {
  height: 100%;
  display: block;
}
</style>
