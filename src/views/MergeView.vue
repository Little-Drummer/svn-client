<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ArchiveRestore, GitMerge, Plus, RefreshCw, RotateCcw, Settings2, Trash2 } from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import TaskOutput from '@/components/TaskOutput.vue'
import { api } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import { confirm } from '../composables/use-confirm-dialog'
import type { MergePreview, MergeRevision, MergeRoute, MergeRouteConfig, Project } from '../types/svn'

const props = defineProps<{ activeProjectName?: string | null }>()

const tasksStore = useTasksStore()
const toast = useErrorToast()

const projects = ref<Project[]>([])
const projectName = ref<string>('')
const routes = ref<MergeRoute[]>([])
const route = ref<MergeRoute | null>(null)
const routeConfigOpen = ref(false)
const routeConfigs = ref<MergeRouteConfig[]>([])
const loadingConfigs = ref(false)
const savingConfigs = ref(false)

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
const mergeTaskId = ref<string | null>(null)
const taskAction = ref<'merge' | 'commit' | null>(null)
const mergeReadyToCommit = ref(false)
const commitCompleted = ref(false)
const recovering = ref(false)
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})
const currentProject = computed(() => projects.value.find((p) => p.name === projectName.value) ?? null)
const mergeTaskFailed = computed(() => {
  if (!mergeTaskId.value) return false
  const task = tasksStore.tasks.get(mergeTaskId.value)
  return !!task?.finished && task.success === false
})
const mergeSessionLocked = computed(() => mergeReadyToCommit.value || mergeTaskFailed.value || commitCompleted.value)
const routeConfigsInvalid = computed(() =>
  routeConfigs.value.some(
    (config) =>
      !config.sourceProjectName ||
      !config.targetProjectName ||
      !config.sourceEnv ||
      !config.sourceModule ||
      !config.targetEnv ||
      !config.targetModule,
  ),
)

function projectByName(name: string) {
  return projects.value.find((project) => project.name === name) ?? null
}

function branchOptions(projectName: string, selected?: string) {
  const names = projectByName(projectName)?.branches.map((branch) => branch.environment) ?? []
  if (selected && !names.includes(selected)) return [...names, selected]
  return names
}

function modulesForEnv(projectName: string, env: string) {
  return projectByName(projectName)?.branches.find((branch) => branch.environment === env)?.modules ?? []
}

function moduleOptions(projectName: string, env: string, selected?: string) {
  const names = modulesForEnv(projectName, env).map((module) => module.module)
  if (selected && !names.includes(selected)) return [...names, selected]
  return names
}

function preferredBranch(projectName: string, name: string) {
  return projectByName(projectName)?.branches.find((branch) => branch.environment === name)?.environment
}

function preferredAnyBranch(projectName: string, names: string[]) {
  for (const name of names) {
    const found = preferredBranch(projectName, name)
    if (found) return found
  }
  return null
}

function preferredModule(projectName: string, env: string, preferred = 'rest') {
  const modules = modulesForEnv(projectName, env)
  return modules.find((module) => module.module === preferred)?.module ?? modules[0]?.module ?? ''
}

function routeConfigLabel(config: MergeRouteConfig) {
  return `${config.sourceProjectName}/${config.sourceEnv}/${config.sourceModule} -> ${config.targetProjectName}/${config.targetEnv}/${config.targetModule}`
}

function syncRouteConfigName(config: MergeRouteConfig) {
  config.name = routeConfigLabel(config)
}

function onConfigEnvChange(config: MergeRouteConfig, side: 'source' | 'target') {
  if (side === 'source') {
    const options = moduleOptions(config.sourceProjectName, config.sourceEnv)
    if (!options.includes(config.sourceModule)) config.sourceModule = preferredModule(config.sourceProjectName, config.sourceEnv)
  } else {
    const options = moduleOptions(config.targetProjectName, config.targetEnv)
    if (!options.includes(config.targetModule)) config.targetModule = preferredModule(config.targetProjectName, config.targetEnv)
  }
  syncRouteConfigName(config)
}

// 切换端点项目后重置为该项目实际存在的分支和模块
function onConfigProjectChange(config: MergeRouteConfig, side: 'source' | 'target') {
  if (side === 'source') {
    config.sourceEnv = branchOptions(config.sourceProjectName)[0] ?? ''
    config.sourceModule = preferredModule(config.sourceProjectName, config.sourceEnv)
  } else {
    config.targetEnv = branchOptions(config.targetProjectName)[0] ?? ''
    config.targetModule = preferredModule(config.targetProjectName, config.targetEnv)
  }
  syncRouteConfigName(config)
}

function hasConfigEndpoint(config: MergeRouteConfig, side: 'source' | 'target') {
  const env = side === 'source' ? config.sourceEnv : config.targetEnv
  const module = side === 'source' ? config.sourceModule : config.targetModule
  const endpointProject = side === 'source' ? config.sourceProjectName : config.targetProjectName
  return modulesForEnv(endpointProject, env).some((item) => item.module === module)
}

function configAvailable(config: MergeRouteConfig) {
  return hasConfigEndpoint(config, 'source') && hasConfigEndpoint(config, 'target')
}

function createRouteConfig(): MergeRouteConfig {
  const sourceEnv =
    preferredAnyBranch(projectName.value, ['produce', 'pro']) ?? currentProject.value?.branches[0]?.environment ?? ''
  const targetEnv =
    preferredBranch(projectName.value, '1.0bugfix') ??
    currentProject.value?.branches.find((branch) => branch.environment !== sourceEnv)?.environment ??
    sourceEnv
  const config: MergeRouteConfig = {
    id: crypto.randomUUID(),
    projectName: projectName.value,
    name: '',
    sourceProjectName: projectName.value,
    sourceEnv,
    sourceModule: preferredModule(projectName.value, sourceEnv),
    targetProjectName: projectName.value,
    targetEnv,
    targetModule: preferredModule(projectName.value, targetEnv),
    enabled: true,
  }
  syncRouteConfigName(config)
  return config
}

async function openRouteConfig() {
  if (!projectName.value) return
  routeConfigOpen.value = true
  loadingConfigs.value = true
  try {
    routeConfigs.value = await api.mergeGetRouteConfigs(projectName.value)
  } catch (e) {
    toast(e, '加载合并方向配置失败')
  } finally {
    loadingConfigs.value = false
  }
}

function addRouteConfig() {
  routeConfigs.value = [...routeConfigs.value, createRouteConfig()]
}

function removeRouteConfig(id: string) {
  routeConfigs.value = routeConfigs.value.filter((config) => config.id !== id)
}

async function saveRouteConfigs() {
  if (!projectName.value) return
  savingConfigs.value = true
  try {
    routeConfigs.value = await api.mergeSaveRouteConfigs(projectName.value, routeConfigs.value)
    routeConfigOpen.value = false
    await onProjectChange()
  } catch (e) {
    toast(e, '保存合并方向配置失败')
  } finally {
    savingConfigs.value = false
  }
}

// 预览区域可拖拽调整高度
const previewHeight = ref(280)
let resizeStartY = 0
let resizeStartH = 0
function startPreviewResize(e: MouseEvent) {
  resizeStartY = e.clientY
  resizeStartH = previewHeight.value
  window.addEventListener('mousemove', onPreviewResize)
  window.addEventListener('mouseup', stopPreviewResize)
}
function onPreviewResize(e: MouseEvent) {
  previewHeight.value = Math.max(140, Math.min(700, resizeStartH + (resizeStartY - e.clientY)))
}
function stopPreviewResize() {
  window.removeEventListener('mousemove', onPreviewResize)
  window.removeEventListener('mouseup', stopPreviewResize)
}
onBeforeUnmount(stopPreviewResize)

function formatDate(d?: string | null): string {
  if (!d) return ''
  try {
    return new Date(d).toLocaleString('zh-CN', {
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
      hour12: false,
    }).replace(/\//g, '-')
  } catch {
    return d.slice(0, 19).replace('T', ' ')
  }
}

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

const handledTaskIds = new Set<string>()

// 合并成功后停在待提交阶段；提交成功后恢复合并前的搁置内容
watch(
  () => tasksStore.completedTask,
  async (completed) => {
    if (
      !completed ||
      completed.taskId !== taskId.value ||
      !completed.success ||
      !route.value ||
      handledTaskIds.has(completed.taskId)
    ) return
    handledTaskIds.add(completed.taskId)
    if (taskAction.value === 'merge') mergeReadyToCommit.value = true
    if (taskAction.value === 'commit') {
      commitCompleted.value = true
      await restoreShelfAfterCommit()
    }
  },
)

async function restoreShelfAfterCommit() {
  if (!route.value || !mergeTaskId.value) return
  recovering.value = true
  try {
    await api.mergeRestoreShelf(mergeTaskId.value, route.value.targetPath)
    mergeReadyToCommit.value = false
    commitCompleted.value = false
    mergeTaskId.value = null
    await fetchRevisions()
  } catch (e) {
    toast(e, '提交成功，但恢复搁置文件失败，请重试恢复')
  } finally {
    recovering.value = false
  }
}

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
    if (!mergeSessionLocked.value && name && projects.value.some((p) => p.name === name) && name !== projectName.value) {
      projectName.value = name
      onProjectChange()
    }
  },
)

async function onProjectChange() {
  routeConfigOpen.value = false
  route.value = null
  routes.value = []
  taskId.value = null
  mergeTaskId.value = null
  taskAction.value = null
  mergeReadyToCommit.value = false
  commitCompleted.value = false
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
  mergeReadyToCommit.value = false
  commitCompleted.value = false
}

function selectRoute(r: MergeRoute) {
  if (running.value || mergeSessionLocked.value) return
  route.value = r
  taskId.value = null
  mergeTaskId.value = null
  taskAction.value = null
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

async function executeMerge() {
  if (!route.value || selected.value.size === 0 || !message.value.trim()) return
  const ok = await confirm({
    title: '执行合并',
    content: `合并方向：${route.value.name}\n\n目标工作副本如有修改将先自动搁置，再执行 update → merge，不会自动提交。共 ${selected.value.size} 个版本。确认执行？`,
    confirmText: '执行合并',
    destructive: true,
  })
  if (!ok) return
  try {
    const revs = [...selected.value].sort((a, b) => a - b)
    taskId.value = await api.mergeExecute(route.value, revs)
    mergeTaskId.value = taskId.value
    taskAction.value = 'merge'
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

async function commitMerge() {
  if (!route.value || !mergeReadyToCommit.value || commitCompleted.value || !message.value.trim()) return
  const ok = await confirm({
    title: '提交合并结果',
    content: `将提交目标工作副本：${route.value.targetPath}\n\n请确认已检查合并结果和冲突。`,
    confirmText: '提交',
    destructive: true,
  })
  if (!ok) return
  try {
    taskId.value = await api.startCommit([route.value.targetPath], message.value)
    taskAction.value = 'commit'
    tasksStore.register({
      taskId: taskId.value,
      kind: 'commit',
      title: `提交合并 ${route.value.name}`,
      command: `svn commit ${route.value.targetPath}`,
    })
  } catch (e) {
    toast(e, '启动提交失败')
  }
}

// 保留合并结果，直接恢复合并前的本地修改，并退出当前提交流程
async function restoreShelfWithoutCommit() {
  if (!route.value || !mergeTaskId.value || !mergeReadyToCommit.value || recovering.value) return
  const ok = await confirm({
    title: '不提交并恢复搁置',
    content: `不会提交或撤销本次合并，将直接恢复合并前搁置的文件。恢复后，合并结果与原本地修改会同时保留在目标工作副本中。\n\n目标：${route.value.targetPath}`,
    confirmText: '直接恢复',
    destructive: true,
  })
  if (!ok) return
  recovering.value = true
  try {
    await api.mergeRestoreShelf(mergeTaskId.value, route.value.targetPath)
    mergeTaskId.value = null
    taskAction.value = null
    resetRevisions()
  } catch (e) {
    toast(e, '恢复搁置文件失败')
  } finally {
    recovering.value = false
  }
}

// 撤销合并现场后再恢复原修改，避免把两部分改动混在一起
async function rollbackAndRestore() {
  if (!route.value || !mergeTaskId.value || recovering.value) return
  const ok = await confirm({
    title: '撤销合并并恢复搁置',
    content: `将还原本次合并在目标工作副本产生的全部修改，然后恢复合并前自动搁置的文件。\n\n目标：${route.value.targetPath}`,
    confirmText: '确认撤销并恢复',
    destructive: true,
  })
  if (!ok) return
  recovering.value = true
  try {
    await api.mergeRollback(mergeTaskId.value, route.value.targetPath)
    taskId.value = null
    mergeTaskId.value = null
    taskAction.value = null
    mergeReadyToCommit.value = false
    commitCompleted.value = false
    await fetchRevisions()
  } catch (e) {
    toast(e, '撤销合并或恢复搁置文件失败')
  } finally {
    recovering.value = false
  }
}
</script>

<template>
  <div class="merge-view">
    <div class="merge-config">
      <!-- 项目 + 方向选择 -->
      <div class="config-row project-row">
        <span class="label">项目</span>
        <select
          v-model="projectName"
          class="ui-select"
          :disabled="running || mergeSessionLocked"
          @change="onProjectChange"
        >
          <option v-for="p in projects" :key="p.name" :value="p.name">{{ p.name }}</option>
        </select>
        <Button
          size="xs"
          variant="ghost"
          class="route-settings-btn"
          :disabled="running || mergeSessionLocked || !projectName"
          @click="openRouteConfig"
        >
          <Settings2 class="settings-icon" />
          <span>配置方向</span>
        </Button>
      </div>

      <div class="routes-block">
        <div class="routes-title">
          <span class="label">合并方向</span>
        </div>
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
            :disabled="running || mergeSessionLocked"
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
        <Button size="xs" :disabled="loadingRevisions || running || mergeSessionLocked" @click="fetchRevisions">
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
          <Button size="xs" variant="ghost" :disabled="running || mergeSessionLocked" @click="selectAllVisible">全选</Button>
          <Button size="xs" variant="ghost" :disabled="running || mergeSessionLocked" @click="clearSelection">清空</Button>
        </template>
        <Button
          v-if="selectedCount > 0"
          size="xs"
          :disabled="generating || running || mergeSessionLocked"
          @click="generatePreview"
        >
          {{ generating ? '生成中…' : '生成合并日志' }}
        </Button>
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
              :disabled="running || mergeSessionLocked"
              @update:model-value="(v: boolean | 'indeterminate') => toggleRev(r.revision, v)"
            />
            <div class="rev-meta">
              <div class="rev-head">
                <span class="rev-num mono">r{{ r.revision }}</span>
                <span class="rev-author">{{ r.author || '—' }}</span>
                <span class="rev-date">{{ formatDate(r.date) }}</span>
              </div>
              <div class="rev-msg">{{ r.message }}</div>
            </div>
          </label>
        </div>
      </div>
    </div>

    <!-- 预览与执行 -->
    <div v-if="route && preview" class="preview-block" :style="{ height: previewHeight + 'px' }">
      <div class="resize-handle" @mousedown.prevent="startPreviewResize" />
      <div class="preview-toolbar">
        <code class="cmd mono">{{ preview.command }}</code>
      </div>
      <Textarea
        v-model="message"
        class="merge-message mono"
        placeholder="合并日志（可编辑）"
      />
      <div class="exec-actions">
        <Badge v-if="commitCompleted" variant="secondary">提交完成，待恢复搁置文件</Badge>
        <Badge v-else-if="mergeReadyToCommit" variant="secondary">合并完成，待检查并提交</Badge>
        <Button
          v-if="mergeReadyToCommit && !commitCompleted"
          variant="ghost"
          :disabled="running || recovering"
          @click="rollbackAndRestore"
        >
          <RotateCcw class="icon-xs" />
          撤销合并并恢复搁置
        </Button>
        <Button
          v-if="mergeReadyToCommit && !commitCompleted"
          variant="secondary"
          :disabled="running || recovering"
          @click="restoreShelfWithoutCommit"
        >
          <ArchiveRestore class="icon-xs" />
          {{ recovering ? '恢复中…' : '不提交，恢复搁置' }}
        </Button>
        <Button
          v-if="commitCompleted"
          variant="secondary"
          :disabled="recovering"
          @click="restoreShelfAfterCommit"
        >
          <ArchiveRestore class="icon-xs" />
          {{ recovering ? '恢复中…' : '恢复搁置文件' }}
        </Button>
        <Button
          :disabled="running || mergeReadyToCommit || !message.trim()"
          @click="executeMerge"
        >
          {{ running && taskAction === 'merge' ? '合并中…' : '执行合并' }}
        </Button>
        <Button
          variant="default"
          :disabled="running || !mergeReadyToCommit || commitCompleted || !message.trim()"
          @click="commitMerge"
        >
          {{ running && taskAction === 'commit' ? '提交中…' : '提交合并结果' }}
        </Button>
      </div>
    </div>

    <TaskOutput
      v-if="taskId"
      :task-id="taskId"
      closable
      @retried="taskId = $event"
      @close="taskId = null"
    />

    <div v-if="mergeTaskFailed && route" class="merge-recovery-bar">
      <Badge variant="destructive">合并失败或存在冲突</Badge>
      <span>可撤销本次合并现场，并恢复合并前自动搁置的文件。</span>
      <Button size="xs" variant="secondary" :disabled="recovering" @click="rollbackAndRestore">
        <RotateCcw class="icon-xs" />
        {{ recovering ? '恢复中…' : '撤销合并并恢复搁置' }}
      </Button>
    </div>

    <Dialog v-model:open="routeConfigOpen">
      <DialogContent class="route-config-dialog">
        <DialogHeader>
          <DialogTitle>配置合并方向</DialogTitle>
        </DialogHeader>

        <div class="route-config-body">
          <div class="route-config-head">
            <span class="mono route-config-project">{{ projectName }}</span>
            <Button
              size="xs"
              variant="secondary"
              :disabled="loadingConfigs || !currentProject?.branches.length"
              @click="addRouteConfig"
            >
              <Plus class="icon-xs" />
              新增方向
            </Button>
          </div>

          <div v-if="loadingConfigs" class="hint center"><LoadingSpinner /> 加载配置中…</div>
          <EmptyState
            v-else-if="routeConfigs.length === 0"
            description="当前项目还没有自定义合并方向"
          />
          <div v-else class="route-config-list">
            <div v-for="config in routeConfigs" :key="config.id" class="route-config-item">
              <div class="route-config-line">
                <Checkbox
                  :model-value="config.enabled"
                  title="启用"
                  @update:model-value="(v) => (config.enabled = v === true)"
                />
                <input
                  v-model="config.name"
                  class="native-input route-name-input"
                  placeholder="方向名称"
                />
                <Badge
                  v-if="!configAvailable(config)"
                  variant="outline"
                  class="status-warning"
                >
                  缺少工作副本
                </Badge>
                <Button size="xs" variant="ghost" class="danger-action" @click="removeRouteConfig(config.id)">
                  <Trash2 class="icon-xs" />
                </Button>
              </div>

              <div class="route-config-grid">
                <label class="route-field">
                  <span>来源项目</span>
                  <select
                    v-model="config.sourceProjectName"
                    class="ui-select"
                    @change="onConfigProjectChange(config, 'source')"
                  >
                    <option v-for="project in projects" :key="`source-project-${config.id}-${project.name}`" :value="project.name">
                      {{ project.name }}
                    </option>
                  </select>
                </label>
                <label class="route-field">
                  <span>来源分支</span>
                  <select
                    v-model="config.sourceEnv"
                    class="ui-select"
                    @change="onConfigEnvChange(config, 'source')"
                  >
                    <option
                      v-for="env in branchOptions(config.sourceProjectName, config.sourceEnv)"
                      :key="`source-env-${config.id}-${env}`"
                      :value="env"
                    >
                      {{ env }}
                    </option>
                  </select>
                </label>
                <label class="route-field">
                  <span>来源模块</span>
                  <select
                    v-model="config.sourceModule"
                    class="ui-select"
                    @change="syncRouteConfigName(config)"
                  >
                    <option
                      v-for="module in moduleOptions(config.sourceProjectName, config.sourceEnv, config.sourceModule)"
                      :key="`source-module-${config.id}-${module}`"
                      :value="module"
                    >
                      {{ module }}
                    </option>
                  </select>
                </label>
                <label class="route-field">
                  <span>目标项目</span>
                  <select
                    v-model="config.targetProjectName"
                    class="ui-select"
                    @change="onConfigProjectChange(config, 'target')"
                  >
                    <option v-for="project in projects" :key="`target-project-${config.id}-${project.name}`" :value="project.name">
                      {{ project.name }}
                    </option>
                  </select>
                </label>
                <label class="route-field">
                  <span>目标分支</span>
                  <select
                    v-model="config.targetEnv"
                    class="ui-select"
                    @change="onConfigEnvChange(config, 'target')"
                  >
                    <option
                      v-for="env in branchOptions(config.targetProjectName, config.targetEnv)"
                      :key="`target-env-${config.id}-${env}`"
                      :value="env"
                    >
                      {{ env }}
                    </option>
                  </select>
                </label>
                <label class="route-field">
                  <span>目标模块</span>
                  <select
                    v-model="config.targetModule"
                    class="ui-select"
                    @change="syncRouteConfigName(config)"
                  >
                    <option
                      v-for="module in moduleOptions(config.targetProjectName, config.targetEnv, config.targetModule)"
                      :key="`target-module-${config.id}-${module}`"
                      :value="module"
                    >
                      {{ module }}
                    </option>
                  </select>
                </label>
              </div>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" :disabled="savingConfigs" @click="routeConfigOpen = false">取消</Button>
          <Button :disabled="savingConfigs || loadingConfigs || routeConfigsInvalid" @click="saveRouteConfigs">
            {{ savingConfigs ? '保存中…' : '保存配置' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
.project-row {
  min-width: 0;
}
.project-row .ui-select {
  flex: 1;
  min-width: 180px;
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
  align-items: flex-start;
}
.routes-title {
  display: flex;
  align-items: center;
  flex: none;
}
.route-settings-btn {
  position: relative;
  flex: none;
  gap: 6px;
  height: 28px;
  padding: 0 11px;
  overflow: hidden;
  white-space: nowrap;
  color: var(--fg);
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--mat-elevated) 92%, white 8%), var(--mat-elevated));
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 55%, transparent),
    0 0 0 1px color-mix(in srgb, var(--accent) 20%, var(--stroke-soft)),
    0 8px 18px color-mix(in srgb, var(--accent) 10%, transparent);
  transition:
    color 160ms ease,
    transform 160ms ease,
    box-shadow 180ms ease,
    background 180ms ease;
}
.route-settings-btn::before {
  content: '';
  position: absolute;
  inset: 1px;
  border-radius: 6px;
  background:
    linear-gradient(110deg, transparent 0%, color-mix(in srgb, white 28%, transparent) 42%, transparent 72%);
  opacity: 0;
  transform: translateX(-24px);
  transition:
    opacity 180ms ease,
    transform 260ms ease;
  pointer-events: none;
}
.route-settings-btn:hover:not(:disabled) {
  color: var(--accent);
  transform: translateY(-1px);
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 60%, transparent),
    0 0 0 1px color-mix(in srgb, var(--accent) 42%, var(--stroke-soft)),
    0 10px 24px color-mix(in srgb, var(--accent) 16%, transparent);
}
.route-settings-btn:hover:not(:disabled)::before {
  opacity: 1;
  transform: translateX(36px);
}
.route-settings-btn:active:not(:disabled) {
  transform: translateY(0);
  box-shadow:
    inset 0 1px 2px color-mix(in srgb, black 10%, transparent),
    0 0 0 1px color-mix(in srgb, var(--accent) 36%, var(--stroke-soft));
}
.settings-icon {
  width: 13px;
  height: 13px;
  transition: transform 180ms ease;
}
.route-settings-btn:hover:not(:disabled) .settings-icon {
  transform: rotate(18deg);
}
@media (prefers-reduced-motion: reduce) {
  .route-settings-btn,
  .route-settings-btn::before,
  .settings-icon {
    transition: none;
  }
  .route-settings-btn:hover:not(:disabled),
  .route-settings-btn:hover:not(:disabled) .settings-icon {
    transform: none;
  }
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
.route-config-dialog {
  width: min(860px, calc(100vw - 48px));
  max-width: none;
}
.route-config-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 260px;
  max-height: min(620px, calc(100vh - 220px));
  overflow: hidden;
}
.route-config-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.route-config-project {
  flex: 1;
  min-width: 0;
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.route-config-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: auto;
  padding-right: 2px;
}
.route-config-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border-radius: 7px;
  background: var(--mat-elevated);
  box-shadow: var(--stroke-control);
}
.route-config-line {
  display: flex;
  align-items: center;
  gap: 8px;
}
.route-name-input {
  max-width: none;
  min-width: 0;
}
.route-config-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(150px, 1fr));
  gap: 8px;
}
.route-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  font-size: var(--fs-caption);
  color: var(--fg-muted);
}
.route-field .ui-select {
  width: 100%;
}
.status-warning {
  color: var(--warning, #9a6700);
  border-color: color-mix(in srgb, currentColor 35%, transparent);
}
.danger-action {
  color: var(--danger, #d1242f);
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
  padding: 8px 12px 10px;
  border-top: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  position: relative;
  flex: none;
  min-height: 140px;
}
.resize-handle {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 5px;
  cursor: ns-resize;
  background: transparent;
  transition: background 120ms;
}
.resize-handle:hover {
  background: var(--accent-soft);
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
  flex: 1;
  min-height: 80px;
  resize: none;
}
.exec-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: flex-end;
}
.merge-recovery-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-top: var(--hairline) solid color-mix(in srgb, var(--danger, #d1242f) 28%, var(--stroke-soft));
  color: var(--danger, #d1242f);
  background: color-mix(in srgb, var(--danger, #d1242f) 7%, var(--mat-content));
  font-size: var(--fs-callout);
}
.merge-recovery-bar > span:nth-child(2) {
  flex: 1;
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
