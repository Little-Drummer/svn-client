<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { FileCheck2 } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import { getDecodedUrl } from '@/lib/utils'
import TaskOutput from './TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry } from '../types/svn'

const props = defineProps<{
  workingCopy: WorkingCopyEntry
  checkedPaths: string[]
  unversionedPaths: string[]
}>()
const emit = defineEmits<{
  done: []
  exclude: [paths: string[]]
}>()

const tasksStore = useTasksStore()
const toast = useErrorToast()

const message = ref('')
const taskId = ref<string | null>(null)
const preparing = ref(false)
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
  () =>
    props.checkedPaths.length > 0 &&
    message.value.trim().length > 0 &&
    !preparing.value &&
    !submitting.value,
)

function shortPath(path: string) {
  const root = props.workingCopy.path
  if (path.startsWith(root)) {
    return path.slice(root.length).replace(/^[\\/]+/, '') || path
  }
  return path
}

function commitTargetLabel() {
  return props.workingCopy.url ? getDecodedUrl(props.workingCopy.url) : props.workingCopy.path
}

async function submit() {
  if (!canCommit.value) return
  let paths = [...props.checkedPaths]
  let unversionedPaths = [...props.unversionedPaths]

  if (unversionedPaths.length > 0) {
    const addUnversioned = await confirm({
      title: '发现未跟踪文件',
      content: `以下文件尚未加入版本控制：\n\n${unversionedPaths.map(shortPath).join('\n')}\n\n请选择 Add 后提交，或从本次提交中去掉。`,
      confirmText: 'Add 并保留',
      cancelText: '从本次提交中去掉',
    })
    // 关闭弹窗表示取消整个提交流程，不改变当前选择。
    if (addUnversioned === null) return
    if (!addUnversioned) {
      const excluded = new Set(unversionedPaths)
      paths = paths.filter((path) => !excluded.has(path))
      emit('exclude', unversionedPaths)
      unversionedPaths = []
      if (paths.length === 0) return
    }
  }

  const ok = await confirm({
    title: '确认提交',
    content: `将提交 ${paths.length} 个文件\n\n目标：${commitTargetLabel()}`,
    confirmText: '提交',
  })
  if (!ok) return
  preparing.value = true
  try {
    // SVN 不会直接提交未跟踪文件，先加入版本控制再使用同一批路径提交。
    if (unversionedPaths.length > 0) {
      await api.add(unversionedPaths)
    }
    taskId.value = await launchCommit(paths, message.value.trim())
  } catch (e) {
    toast(e, '启动提交失败')
  } finally {
    preparing.value = false
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
        <div class="commit-empty">
          <EmptyState description="勾选左侧文件后再提交" />
        </div>
      </template>
      <template v-else>
        <span class="summary-count">{{ checkedPaths.length }}</span>
        <span class="summary-label">个文件待提交</span>
      </template>
    </div>

    <div v-if="checkedPaths.length > 0" class="selected-files">
      <div
        v-for="path in checkedPaths"
        :key="path"
        class="selected-file"
        :title="path"
      >
        <FileCheck2 class="selected-file-icon" />
        <span class="selected-file-name mono">{{ shortPath(path) }}</span>
      </div>
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
        {{ preparing ? '准备中' : submitting ? '提交中' : '提交' }}
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
  justify-content: center;
  gap: 6px;
  font-size: var(--fs-body);
  min-height: 22px;
}
.commit-empty {
  width: 100%;
}
.commit-empty :deep(.empty-state) {
  width: 100%;
  padding: 36px 0 30px;
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
.selected-files {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: none;
  max-height: 132px;
  overflow: auto;
  padding: 6px;
  border: var(--hairline) solid var(--stroke-soft);
  border-radius: var(--radius-control);
  background: color-mix(in srgb, var(--fg) 4%, transparent);
}
.selected-file {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 22px;
  padding: 2px 4px;
  border-radius: var(--radius-sm);
  color: var(--fg);
}
.selected-file:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.selected-file-icon {
  width: 13px;
  height: 13px;
  flex: none;
  color: var(--accent);
}
.selected-file-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-caption);
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
