<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { GitMerge, RefreshCw } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import TaskOutput from '@/components/TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import { confirm } from '../composables/use-confirm-dialog'
import type { MergePreview, MergeRevision, MergeRoute, Project } from '../types/svn'

const props = defineProps<{ activeProjectName?: string | null }>()

const tasksStore = useTasksStore()
const toast = useErrorToast()

const projects = ref<Project[]>([])
const projectName = ref<string>('')
const routes = ref<MergeRoute[]>([])
const route = ref<MergeRoute | null>(null)

const loadingRoutes = ref(false)
const loadingRevisions = ref(false)
const revisions = ref<MergeRevision[]>([])
const selected = ref<Set<number>>(new Set())
const filter = ref('')
const fetched = ref(false)

const preview = ref<MergePreview | null>(null)
const message = ref('')
const generating = ref(false)

const taskId = ref<string | null>(null)
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

const filteredRevisions = computed(() => {
  const kw = filter.value.trim().toLowerCase()
  if (!kw) return revisions.value
  return revisions.value.filter(
    (r) =>
      String(r.revision).includes(kw) ||
      (r.author ?? '').toLowerCase().includes(kw) ||
      r.message.toLowerCase().includes(kw),
  )
})

const selectedCount = computed(() => selected.value.size)

onMounted(async () => {
  tasksStore.ensureListener()
  await loadProjects()
})

async function loadProjects() {
  try {
    projects.value = await api.listProjects()
    // 优先选中外部传入的当前项目，否则取第一个
    const want = props.activeProjectName
    const match = want && projects.value.find((p) => p.name === want)
    projectName.value = match ? want! : projects.value[0]?.name ?? ''
    await onProjectChange()
  } catch (e) {
    toast(e, '加载项目失败')
  }
}

watch(
  () => props.activeProjectName,
  (name) => {
    if (name && projects.value.some((p) => p.name === name) && name !== projectName.value) {
      projectName.value = name
      onProjectChange()
    }
  },
)

async function onProjectChange() {
  route.value = null
  routes.value = []
  resetRevisions()
  if (!projectName.value) return
  loadingRoutes.value = true
  try {
    routes.value = await api.mergeListRoutes(projectName.value)
  } catch (e) {
    toast(e, '加载合并方向失败')
  } finally {
    loadingRoutes.value = false
  }
}

function resetRevisions() {
  revisions.value = []
  selected.value = new Set()
  filter.value = ''
  fetched.value = false
  preview.value = null
  message.value = ''
}

function selectRoute(r: MergeRoute) {
  if (running.value) return
  route.value = r
  resetRevisions()
}

async function fetchRevisions() {
  if (!route.value) return
  loadingRevisions.value = true
  fetched.value = false
  try {
    revisions.value = await api.mergeFetchRevisions(route.value)
    selected.value = new Set()
    preview.value = null
    message.value = ''
    fetched.value = true
  } catch (e) {
    toast(e, '拉取可合并版本失败')
  } finally {
    loadingRevisions.value = false
  }
}

function toggleRev(rev: number, checked: boolean | 'indeterminate') {
  const next = new Set(selected.value)
  if (checked === true) next.add(rev)
  else next.delete(rev)
  selected.value = next
  preview.value = null
}

function selectAllVisible() {
  const next = new Set(selected.value)
  for (const r of filteredRevisions.value) next.add(r.revision)
  selected.value = next
  preview.value = null
}

function clearSelection() {
  selected.value = new Set()
  preview.value = null
}

async function generatePreview() {
  if (!route.value || selected.value.size === 0) return
  generating.value = true
  try {
    const revs = [...selected.value].sort((a, b) => a - b)
    preview.value = await api.mergePreview(route.value, revisions.value, revs)
    message.value = preview.value.message
  } catch (e) {
    toast(e, '生成合并日志失败')
  } finally {
    generating.value = false
  }
}

async function execute() {
  if (!route.value || selected.value.size === 0 || !message.value.trim()) return
  const ok = await confirm({
    title: '执行合并',
    content: `将对目标工作副本执行 update → merge → commit${
      route.value.syncBranch ? '（必要时先搁置本地修改并在提交后恢复）' : ''
    }。共 ${selected.value.size} 个版本。确认执行？`,
    confirmText: '执行合并',
    destructive: true,
  })
  if (!ok) return
  try {
    const revs = [...selected.value].sort((a, b) => a - b)
    taskId.value = await api.mergeExecute(route.value, revs, message.value)
    tasksStore.register({
      taskId: taskId.value,
      kind: 'merge',
      title: `合并 ${route.value.name}`,
      command: preview.value?.command,
    })
  } catch (e) {
    toast(e, '启动合并失败')
  }
}
</script>

<template>
  <div class="merge-view">
    <div class="merge-config">
      <!-- 项目 + 方向选择 -->
      <div class="config-row">
        <span class="label">项目</span>
        <select
          v-model="projectName"
          class="ui-select"
          :disabled="running"
          @change="onProjectChange"
        >
          <option v-for="p in projects" :key="p.name" :value="p.name">{{ p.name }}</option>
        </select>
      </div>

      <div class="routes-block">
        <span class="label">合并方向</span>
        <div v-if="loadingRoutes" class="hint"><LoadingSpinner /> 识别方向中…</div>
        <div v-else-if="routes.length === 0" class="hint">
          该项目没有可识别的合并方向（需要 develop/test/produce 或个人分支）
        </div>
        <div v-else class="routes">
          <button
            v-for="r in routes"
            :key="r.id"
            type="button"
            :class="['route-chip', { active: route?.id === r.id }]"
            :disabled="running"
            @click="selectRoute(r)"
          >
            <GitMerge class="chip-icon" />
            {{ r.name }}
          </button>
        </div>
      </div>
    </div>

    <!-- 版本选择 -->
    <div v-if="route" class="revisions-block">
      <div class="rev-toolbar">
        <Button size="xs" :disabled="loadingRevisions || running" @click="fetchRevisions">
          <RefreshCw class="icon-xs" :class="{ spin: loadingRevisions }" />
          {{ fetched ? '重新拉取' : '拉取可合并版本' }}
        </Button>
        <input
          v-if="fetched && revisions.length"
          v-model="filter"
          class="native-input"
          placeholder="过滤 版本号 / 作者 / 说明"
        />
        <span class="spacer" />
        <template v-if="fetched && revisions.length">
          <Badge variant="secondary">已选 {{ selectedCount }}</Badge>
          <Button size="xs" variant="ghost" @click="selectAllVisible">全选</Button>
          <Button size="xs" variant="ghost" @click="clearSelection">清空</Button>
        </template>
      </div>

      <div class="rev-list-wrap">
        <div v-if="loadingRevisions" class="hint center"><LoadingSpinner /> 分析目标分支尚未包含的版本…</div>
        <EmptyState
          v-else-if="fetched && revisions.length === 0"
          description="没有需要合并的版本（目标分支已包含全部源分支提交）"
        />
        <EmptyState v-else-if="!fetched" description="点击「拉取可合并版本」获取候选提交" />
        <div v-else class="rev-list">
          <label
            v-for="r in filteredRevisions"
            :key="r.revision"
            :class="['rev-item', { checked: selected.has(r.revision) }]"
          >
            <Checkbox
              :model-value="selected.has(r.revision)"
              @update:model-value="(v: boolean | 'indeterminate') => toggleRev(r.revision, v)"
            />
            <div class="rev-meta">
              <div class="rev-head">
                <span class="rev-num mono">r{{ r.revision }}</span>
                <span class="rev-author">{{ r.author || '—' }}</span>
                <span class="rev-date">{{ (r.date || '').slice(0, 19).replace('T', ' ') }}</span>
              </div>
              <div class="rev-msg">{{ r.message }}</div>
            </div>
          </label>
        </div>
      </div>
    </div>

    <!-- 预览与执行 -->
    <div v-if="route && selectedCount > 0" class="preview-block">
      <div class="preview-toolbar">
        <Button size="xs" variant="ghost" :disabled="generating" @click="generatePreview">
          生成合并日志
        </Button>
        <code v-if="preview" class="cmd mono">{{ preview.command }}</code>
      </div>
      <Textarea
        v-if="preview"
        v-model="message"
        class="merge-message mono"
        placeholder="合并日志（可编辑）"
      />
      <div v-if="preview" class="exec-actions">
        <Button :disabled="running || !message.trim()" @click="execute">
          {{ running ? '合并中…' : '执行合并' }}
        </Button>
      </div>
    </div>

    <TaskOutput v-if="taskId" :task-id="taskId" @retried="taskId = $event" />
  </div>
</template>

<style scoped>
.merge-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
  overflow: hidden;
}
.merge-config {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.config-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.label {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  min-width: 56px;
  font-weight: 500;
}
.routes-block {
  display: flex;
  gap: 10px;
}
.routes {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex: 1;
}
.route-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  font-size: var(--fs-callout);
  border-radius: 7px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
  color: var(--fg-muted);
  cursor: pointer;
  transition: all 140ms ease-out;
}
.route-chip:hover:not(:disabled) {
  color: var(--fg-strong);
}
.route-chip.active {
  background: var(--accent-soft);
  color: var(--accent);
  box-shadow: var(--stroke-accent);
}
.route-chip:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.chip-icon {
  width: 13px;
  height: 13px;
}
/* select 外观由全局 .ui-select（macOS 弹出按钮）提供，这里只管布局 */
.ui-select {
  flex: 1;
  min-width: 0;
}
.native-input {
  flex: 1;
  height: 26px;
  padding: 0 8px;
  font-size: var(--fs-callout);
  border-radius: 6px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
  color: var(--fg);
  border: 0;
  outline: none;
}
.native-input {
  max-width: 260px;
}
.revisions-block {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}
.rev-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.spacer {
  flex: 1;
}
.rev-list-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.rev-list {
  display: flex;
  flex-direction: column;
}
.rev-item {
  display: flex;
  gap: 9px;
  padding: 8px 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  cursor: pointer;
  align-items: flex-start;
}
.rev-item:hover {
  background: var(--mat-hover, color-mix(in srgb, var(--mat-content) 92%, var(--fg) 8%));
}
.rev-item.checked {
  background: var(--accent-soft);
}
.rev-meta {
  min-width: 0;
  flex: 1;
}
.rev-head {
  display: flex;
  gap: 10px;
  align-items: baseline;
  font-size: var(--fs-caption);
}
.rev-num {
  color: var(--accent);
  font-weight: 600;
}
.rev-author {
  color: var(--fg);
}
.rev-date {
  color: var(--fg-subtle);
}
.rev-msg {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
  white-space: pre-wrap;
  word-break: break-word;
  margin-top: 2px;
}
.hint {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.hint.center {
  justify-content: center;
  padding: 24px;
}
.preview-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-top: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
}
.preview-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.cmd {
  flex: 1;
  min-width: 0;
  font-size: var(--fs-mono);
  color: var(--fg-subtle);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.merge-message {
  min-height: 120px;
  max-height: 220px;
}
.exec-actions {
  display: flex;
  justify-content: flex-end;
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
</style>
