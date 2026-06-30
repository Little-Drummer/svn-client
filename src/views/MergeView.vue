<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { GitMerge, Plus, RefreshCw, Settings2, Trash2 } from 'lucide-vue-next'

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
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})
const currentProject = computed(() => projects.value.find((p) => p.name === projectName.value) ?? null)
const routeConfigsInvalid = computed(() =>
  routeConfigs.value.some(
    (config) =>
      !config.sourceEnv ||
      !config.sourceModule ||
      !config.targetEnv ||
      !config.targetModule,
  ),
)

function branchOptions(selected?: string) {
  const names = currentProject.value?.branches.map((branch) => branch.environment) ?? []
  if (selected && !names.includes(selected)) return [...names, selected]
  return names
}

function modulesForEnv(env: string) {
  return currentProject.value?.branches.find((branch) => branch.environment === env)?.modules ?? []
}

function moduleOptions(env: string, selected?: string) {
  const names = modulesForEnv(env).map((module) => module.module)
  if (selected && !names.includes(selected)) return [...names, selected]
  return names
}

function preferredBranch(name: string) {
  return currentProject.value?.branches.find((branch) => branch.environment === name)?.environment
}

function preferredAnyBranch(names: string[]) {
  for (const name of names) {
    const found = preferredBranch(name)
    if (found) return found
  }
  return null
}

function preferredModule(env: string, preferred = 'rest') {
  const modules = modulesForEnv(env)
  return modules.find((module) => module.module === preferred)?.module ?? modules[0]?.module ?? ''
}

function routeConfigLabel(config: MergeRouteConfig) {
  return `${config.sourceEnv}/${config.sourceModule} -> ${config.targetEnv}/${config.targetModule}`
}

function syncRouteConfigName(config: MergeRouteConfig) {
  config.name = routeConfigLabel(config)
}

function onConfigEnvChange(config: MergeRouteConfig, side: 'source' | 'target') {
  if (side === 'source') {
    const options = moduleOptions(config.sourceEnv)
    if (!options.includes(config.sourceModule)) config.sourceModule = preferredModule(config.sourceEnv)
  } else {
    const options = moduleOptions(config.targetEnv)
    if (!options.includes(config.targetModule)) config.targetModule = preferredModule(config.targetEnv)
  }
  syncRouteConfigName(config)
}

function hasConfigEndpoint(config: MergeRouteConfig, side: 'source' | 'target') {
  const env = side === 'source' ? config.sourceEnv : config.targetEnv
  const module = side === 'source' ? config.sourceModule : config.targetModule
  return modulesForEnv(env).some((item) => item.module === module)
}

function configAvailable(config: MergeRouteConfig) {
  return hasConfigEndpoint(config, 'source') && hasConfigEndpoint(config, 'target')
}

function createRouteConfig(): MergeRouteConfig {
  const sourceEnv =
    preferredAnyBranch(['produce', 'pro']) ?? currentProject.value?.branches[0]?.environment ?? ''
  const targetEnv =
    preferredBranch('1.0bugfix') ??
    currentProject.value?.branches.find((branch) => branch.environment !== sourceEnv)?.environment ??
    sourceEnv
  const config: MergeRouteConfig = {
    id: crypto.randomUUID(),
    projectName: projectName.value,
    name: '',
    sourceEnv,
    sourceModule: preferredModule(sourceEnv),
    targetEnv,
    targetModule: preferredModule(targetEnv),
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

// 合并成功后自动刷新版本列表
watch(
  () => taskId.value ? tasksStore.tasks.get(taskId.value) : null,
  (t) => {
    if (t?.finished && t.success && route.value) {
      fetchRevisions()
    }
  },
)

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
  routeConfigOpen.value = false
  route.value = null
  routes.value = []
  taskId.value = null
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
  taskId.value = null
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
    content: `合并方向：${route.value.name}\n\n将对目标工作副本执行 update → merge → commit${
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
      <div class="config-row project-row">
        <span class="label">项目</span>
        <select
          v-model="projectName"
          class="ui-select"
          :disabled="running"
          @change="onProjectChange"
        >
          <option v-for="p in projects" :key="p.name" :value="p.name">{{ p.name }}</option>
        </select>
        <Button
          size="xs"
          variant="ghost"
          class="route-settings-btn"
          :disabled="running || !projectName"
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
        <Button
          v-if="selectedCount > 0"
          size="xs"
          :disabled="generating"
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
        <Button :disabled="running || !message.trim()" @click="execute">
          {{ running ? '合并中…' : '执行合并' }}
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
                  <span>来源分支</span>
                  <select
                    v-model="config.sourceEnv"
                    class="ui-select"
                    @change="onConfigEnvChange(config, 'source')"
                  >
                    <option
                      v-for="env in branchOptions(config.sourceEnv)"
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
                      v-for="module in moduleOptions(config.sourceEnv, config.sourceModule)"
                      :key="`source-module-${config.id}-${module}`"
                      :value="module"
                    >
                      {{ module }}
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
                      v-for="env in branchOptions(config.targetEnv)"
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
                      v-for="module in moduleOptions(config.targetEnv, config.targetModule)"
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
  grid-template-columns: repeat(4, minmax(120px, 1fr));
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
