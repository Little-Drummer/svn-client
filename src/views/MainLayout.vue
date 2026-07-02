<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'

import WorkingCopyList from '../components/WorkingCopyList.vue'
import RepositoryList from '../components/RepositoryList.vue'
import StatusView from '../views/StatusView.vue'
import LogView from '../views/LogView.vue'
import MergeView from '../views/MergeView.vue'
import PackageView from '../views/PackageView.vue'
import ConfigPresetView from '../views/ConfigPresetView.vue'
import WorkflowBoardView from '../views/WorkflowBoardView.vue'
import RemoteBrowserView from '../views/RemoteBrowserView.vue'
import CheckoutView from '../views/CheckoutView.vue'
import EnvWarning from '../components/EnvWarning.vue'
import TaskCenter from '../components/TaskCenter.vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { useAppToast } from '@/composables/use-app-toast'
import { useRepositoriesStore } from '../stores/repositories'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { api, describeError } from '../api/svn'
import { getSmartLabel, getLocalProject } from '../lib/utils'
import type { LogTarget, RepositoryEntry } from '../types/svn'

const repoStore = useRepositoriesStore()
const wcStore = useWorkingCopiesStore()
const tab = ref<'overview' | 'status' | 'log' | 'merge' | 'package' | 'config'>('status')
// 从总览看板跳转时临时指定项目，覆盖「跟随选中工作副本」的默认行为
const overrideProject = ref<string | null>(null)
const toast = useAppToast()

// 侧栏「远端仓库」区高度可拖拽调整：远端少、本地多时把空间让给工作副本列表
const REPO_PANE_KEY = 'sidebar.repoPaneHeight'
const sidebarRef = ref<HTMLElement | null>(null)
const repoPaneHeight = ref<number>(Number(localStorage.getItem(REPO_PANE_KEY)) || 260)
const resizingSidebar = ref(false)

function startSidebarResize(e: MouseEvent) {
  e.preventDefault()
  resizingSidebar.value = true
  const startY = e.clientY
  const startH = repoPaneHeight.value
  const sidebarH = sidebarRef.value?.clientHeight ?? 640
  const MIN = 120
  // 上限：扣掉顶部 chrome 与工作副本区的最小可视高度，避免把下方挤没
  const MAX = Math.max(MIN, sidebarH - 52 - 160)

  function onMove(ev: MouseEvent) {
    repoPaneHeight.value = Math.min(MAX, Math.max(MIN, startH + (ev.clientY - startY)))
  }
  function onUp() {
    resizingSidebar.value = false
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    localStorage.setItem(REPO_PANE_KEY, String(Math.round(repoPaneHeight.value)))
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  document.body.style.cursor = 'row-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

const svnVersion = ref<string | null>(null)
const svnError = ref<string | null>(null)

async function detectSvn() {
  svnError.value = null
  svnVersion.value = null
  try {
    svnVersion.value = await api.checkEnvironment()
  } catch (e) {
    svnError.value = describeError(e)
  }
}

// 原生菜单与窗口拖拽的清理句柄
let unlistenMenu: UnlistenFn | null = null
let unlistenDrop: (() => void) | null = null

onMounted(async () => {
  detectSvn()

  // 原生应用菜单的「视图/刷新」项通过 menu-action 事件驱动前端
  unlistenMenu = await listen<string>('menu-action', (event) => {
    const id = event.payload
    switch (id) {
      case 'view:status':
        tab.value = 'status'
        break
      case 'view:log':
        tab.value = 'log'
        break
      case 'action:refresh':
        refreshSelected()
        break
    }
  })

  // 拖拽文件夹到窗口即尝试添加为工作副本
  unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
    if (event.payload.type !== 'drop') return
    const paths = event.payload.paths ?? []
    let added = 0
    for (const p of paths) {
      try {
        await wcStore.add(p)
        added += 1
      } catch (e) {
        toast.error('无法添加', describeError(e))
      }
    }
    if (added > 0) {
      tab.value = 'status'
      toast.success(`已添加 ${added} 个工作副本`)
    }
  })
})

onUnmounted(() => {
  unlistenMenu?.()
  unlistenDrop?.()
})

const selected = computed(() => wcStore.selected)
// 选中远端仓库时主区域切到远端浏览（与工作副本互斥）
const selectedRepo = computed(() => repoStore.selected)

// 路径不可用（外置卷未挂载/目录被删）：状态与历史视图降级为提示页，避免逐个命令报错
const selectedUnavailable = computed(() => !!selected.value && selected.value.available === false)

const recheckingPath = ref(false)
async function recheckAvailability() {
  recheckingPath.value = true
  try {
    await wcStore.reload()
    if (wcStore.selected && wcStore.selected.available !== false) {
      toast.success('路径已恢复可用')
    } else {
      toast.error('路径仍不可用', '请确认磁盘卷已挂载')
    }
  } catch (e) {
    toast.error('检查失败', describeError(e))
  } finally {
    recheckingPath.value = false
  }
}

// 检出弹窗：远端浏览里点「检出当前目录」时打开
const checkoutOpen = ref(false)
const checkoutRepo = ref<RepositoryEntry | null>(null)

function onRemoteCheckout(repo: RepositoryEntry) {
  checkoutRepo.value = repo
  checkoutOpen.value = true
}

// 当前选中工作副本所属项目名，用于合并/打包/配置面板自动预选项目；
// 总览看板跳转时用 overrideProject 临时指定。
const activeProjectName = computed(
  () => overrideProject.value ?? (selected.value ? getLocalProject(selected.value.path) : null),
)

// 总览看板点「合并/打包/配置」：切到对应 tab 并把目标项目带过去
function onBoardNavigate(target: 'merge' | 'package' | 'config', projectName: string) {
  overrideProject.value = projectName
  tab.value = target
}

// 面包屑片段：从工作副本的 URL / repositoryRoot 拆出可读层级
const breadcrumb = computed(() => {
  const wc = selected.value
  if (!wc) return null
  const root = wc.repositoryRoot ?? wc.url ?? ''
  let repoName = ''
  try {
    const u = new URL(root)
    repoName = u.pathname.split('/').filter(Boolean).pop() || u.host
  } catch {
    repoName = root.split(/[\\/]/).filter(Boolean).pop() || root
  }
  // 使用智能标签（displayName 或本地+分支自动推断），而不是只显示最后一段文件夹
  const wcName = getSmartLabel(wc)
  return { repoName, wcName, fullPath: wc.path, revision: wc.revision }
})

// 历史视图的目标：跟随选中工作副本，带上当前 revision 用于「当前副本」标记
const wcLogTarget = computed<LogTarget | null>(() => {
  const wc = selected.value
  if (!wc) return null
  return {
    kind: 'wc',
    target: wc.path,
    title: getSmartLabel(wc),
    repositoryRoot: wc.repositoryRoot ?? null,
    currentRevision: wc.revision ?? null,
  }
})

// 工作副本右键「查看日志」：选中该副本并切到历史 tab
function onViewLog(id: string) {
  repoStore.select(null)
  wcStore.select(id)
  overrideProject.value = null
  tab.value = 'log'
}

async function refreshSelected() {
  if (!selected.value) return
  try {
    await wcStore.refresh(selected.value.id)
    toast.success('已刷新')
  } catch (e) {
    toast.error('刷新失败', describeError(e))
  }
}

async function refreshSelectedRevisionForLog() {
  const wc = selected.value
  if (tab.value !== 'log' || !wc || selectedUnavailable.value) return
  try {
    // 进入历史页签时刷新 mixed revision 摘要，避免使用配置里旧的根目录 revision 打标。
    await wcStore.refresh(wc.id)
  } catch {
    // 历史本身仍可按已有目标加载；这里只是不阻断页面。
  }
}

watch(
  [() => tab.value, () => selected.value?.id],
  () => {
    void refreshSelectedRevisionForLog()
  },
)

function onWorkingCopySelect() {
  repoStore.select(null)
  // 用户手动选了工作副本，恢复「面板跟随选中项目」
  overrideProject.value = null
}

// 选中远端仓库时清掉工作副本选中，保持两侧焦点互斥
function onRepositorySelect() {
  wcStore.select(null)
}
</script>

<template>
  <div class="layout">
    <aside ref="sidebarRef" class="sidebar">
      <div class="sidebar-chrome" data-tauri-drag-region />
      <RepositoryList
        class="repo-pane"
        :style="{ height: `${repoPaneHeight}px` }"
        @select="onRepositorySelect"
      />
      <div
        class="sidebar-resizer"
        :class="{ resizing: resizingSidebar }"
        @mousedown="startSidebarResize"
      />
      <WorkingCopyList
        :show-active="tab === 'status' || tab === 'log'"
        @select="onWorkingCopySelect"
        @view-log="onViewLog"
      />
    </aside>

    <main class="main">
      <!-- 顶栏：左侧让出 traffic light，整体可拖拽 -->
      <header class="topbar" data-tauri-drag-region>
        <div class="topbar-left">
          <div class="breadcrumb" v-if="selectedRepo">
            <span class="crumb crumb-muted">远端仓库</span>
            <span class="crumb-sep">›</span>
            <span class="crumb crumb-strong" :title="selectedRepo.url">
              {{ selectedRepo.name }}
            </span>
          </div>
          <div class="breadcrumb" v-else-if="breadcrumb">
            <span class="crumb crumb-muted">{{ breadcrumb.repoName }}</span>
            <span class="crumb-sep">›</span>
            <span class="crumb crumb-strong" :title="breadcrumb.fullPath">
              {{ breadcrumb.wcName }}
            </span>
            <Badge v-if="breadcrumb.revision" class="rev-badge mono">
              r{{ breadcrumb.revision }}
            </Badge>
          </div>
          <span v-else class="breadcrumb-empty">未选择工作副本</span>
        </div>

        <!-- 中段：segmented control 风格的 tabs（远端浏览时无关，隐藏） -->
        <Tabs v-show="!selectedRepo" v-model="tab" class="topbar-segmented" data-tauri-drag-region="false">
          <TabsList class="seg-list">
            <TabsTrigger value="overview" class="seg-trigger">总览</TabsTrigger>
            <TabsTrigger value="status" class="seg-trigger">状态</TabsTrigger>
            <TabsTrigger value="log" class="seg-trigger">历史</TabsTrigger>
            <TabsTrigger value="merge" class="seg-trigger">合并</TabsTrigger>
            <TabsTrigger value="package" class="seg-trigger">打包</TabsTrigger>
            <TabsTrigger value="config" class="seg-trigger">配置</TabsTrigger>
          </TabsList>
        </Tabs>

        <div class="topbar-right" data-tauri-drag-region="false">
          <Button
            v-if="selected"
            size="sm"
            variant="ghost"
            class="topbar-btn"
            @click="refreshSelected"
          >
            <RefreshCw class="icon-sm" />
            刷新
          </Button>
        </div>
      </header>

      <EnvWarning v-if="svnError" :message="svnError" @retry="detectSvn" />

      <!-- 远端浏览：选中远端仓库时接管内容区 -->
      <RemoteBrowserView
        v-if="selectedRepo"
        :repository="selectedRepo"
        class="content-tabs"
        @checkout="onRemoteCheckout"
      />

      <!-- 内容区：force-mount 让常用 tab 持续挂载，切换不重建。 -->
      <!-- 非活跃 tab 通过 [data-state='inactive'] { display:none } 隐藏。 -->
      <Tabs v-show="!selectedRepo" v-model="tab" class="content-tabs">
        <TabsContent value="overview" class="tab-pane" force-mount>
          <WorkflowBoardView @navigate="onBoardNavigate" />
        </TabsContent>
        <TabsContent value="status" class="tab-pane" force-mount>
          <StatusView
            v-if="selected && !selectedUnavailable"
            :working-copy="selected"
            @view-log="tab = 'log'"
          />
          <div v-else-if="selectedUnavailable" class="empty-pane unavailable-pane">
            <p class="unavailable-title">工作副本路径不可用</p>
            <p class="mono unavailable-path">{{ selected?.path }}</p>
            <p>磁盘卷可能未挂载或目录已被移动，挂载后点击重新检查。</p>
            <Button size="sm" variant="outline" :disabled="recheckingPath" @click="recheckAvailability">
              {{ recheckingPath ? '检查中…' : '重新检查' }}
            </Button>
          </div>
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </TabsContent>
        <TabsContent value="log" class="tab-pane" force-mount>
          <LogView v-if="wcLogTarget && !selectedUnavailable" :target="wcLogTarget" />
          <div v-else-if="selectedUnavailable" class="empty-pane unavailable-pane">
            <p class="unavailable-title">工作副本路径不可用</p>
            <p class="mono unavailable-path">{{ selected?.path }}</p>
            <p>磁盘卷可能未挂载或目录已被移动，挂载后点击重新检查。</p>
            <Button size="sm" variant="outline" :disabled="recheckingPath" @click="recheckAvailability">
              {{ recheckingPath ? '检查中…' : '重新检查' }}
            </Button>
          </div>
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </TabsContent>
        <TabsContent value="merge" class="tab-pane" force-mount>
          <MergeView :active-project-name="activeProjectName" />
        </TabsContent>
        <TabsContent value="package" class="tab-pane" force-mount>
          <PackageView :active-project-name="activeProjectName" />
        </TabsContent>
        <TabsContent value="config" class="tab-pane" force-mount>
          <ConfigPresetView :active-project-name="activeProjectName" />
        </TabsContent>
      </Tabs>

      <footer class="statusbar">
        <span
          class="health-dot"
          :class="{
            'health-ok': svnVersion,
            'health-err': svnError,
            'health-pending': !svnVersion && !svnError,
          }"
        />
        <TooltipProvider v-if="svnVersion">
          <Tooltip>
            <TooltipTrigger as-child>
              <span class="statusbar-text mono">
                svn · {{ svnVersion.split(/\s+/)[0] || svnVersion }}
              </span>
            </TooltipTrigger>
            <TooltipContent>{{ svnVersion }}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        <span v-else-if="svnError" class="statusbar-text statusbar-err">svn 不可用</span>
        <span v-else class="statusbar-text statusbar-muted">检测 svn 中…</span>

        <span class="statusbar-spacer" />
        <TaskCenter />
      </footer>
    </main>

    <!-- 检出弹窗：远端浏览触发，v-if 让每次打开都拿到最新目录预填 -->
    <Dialog v-model:open="checkoutOpen">
      <DialogContent class="checkout-dialog">
        <DialogHeader>
          <DialogTitle>检出工作副本</DialogTitle>
        </DialogHeader>
        <CheckoutView v-if="checkoutOpen" :repository="checkoutRepo" />
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: 248px 1fr;
  /* 锁死唯一一行为容器高度，否则 auto 行会被内容撑大、内层 overflow 永不触发 */
  grid-template-rows: minmax(0, 1fr);
  height: 100vh;
  width: 100vw;
  background: var(--mat-window);
  color: var(--fg);
}

/* ============ Sidebar ============ */
.sidebar {
  display: flex;
  flex-direction: column;
  background: var(--mat-sidebar);
  backdrop-filter: var(--vibrancy-sidebar);
  -webkit-backdrop-filter: var(--vibrancy-sidebar);
  border-right: var(--hairline) solid var(--stroke);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
/* 侧栏顶部留出与 topbar 等高的"窗口拖拽 + 红绿灯"区域 */
.sidebar-chrome {
  height: 52px;
  flex: none;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: transparent;
}

/* 远端仓库区：高度交给拖拽变量控制，覆盖组件自身的 flex/百分比约束 */
.sidebar .repo-pane {
  flex: 0 0 auto;
  min-height: 0;
  max-height: none;
}

/* 可拖拽分隔条：平时只露一条细线，hover / 拖拽时高亮 */
.sidebar-resizer {
  flex: none;
  height: 9px;
  margin-top: -5px;
  cursor: row-resize;
  position: relative;
  z-index: 2;
}
.sidebar-resizer::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: 2px;
  transform: translateY(-50%);
  background: transparent;
  transition: background-color 120ms ease-out;
}
.sidebar-resizer:hover::after,
.sidebar-resizer.resizing::after {
  background: var(--accent);
}

/* ============ Main ============ */
.main {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  /* 允许被压到 grid 行高，让内层 content-tabs 接管滚动而不是把自己撑溢出 */
  min-height: 0;
  background: var(--mat-content);
}

/* ============ Topbar ============ */
.topbar {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 52px;
  flex: none;
  /* 左侧留 78px 给 traffic-light（macOS overlay 模式下需要） */
  padding: 0 12px 0 0;
  border-bottom: var(--hairline) solid var(--stroke);
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
  user-select: none;
}
.topbar-left {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding-left: 12px;
}
.topbar-right {
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
}
.topbar-btn {
  gap: 5px;
  color: var(--fg);
}
.topbar-btn:hover {
  color: var(--fg-strong);
}
.icon-sm {
  width: 13px;
  height: 13px;
}

/* ============ Breadcrumb ============ */
.breadcrumb {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: var(--fs-body);
}
.crumb {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 28vw;
}
.crumb-muted {
  color: var(--fg-muted);
  font-weight: 500;
}
.crumb-strong {
  color: var(--fg-strong);
  font-weight: 600;
}
.crumb-sep {
  color: var(--fg-subtle);
  font-weight: 400;
  font-size: 13px;
  user-select: none;
}
.breadcrumb-empty {
  color: var(--fg-muted);
  font-size: var(--fs-body);
}
.rev-badge {
  margin-left: 4px;
  height: 18px;
  padding: 0 7px;
  font-size: var(--fs-caption);
  font-weight: 500;
  border-radius: var(--radius-pill);
  background: var(--accent-soft);
  color: var(--accent);
  border: var(--hairline) solid color-mix(in srgb, var(--accent) 30%, transparent);
}

/* ============ Segmented Control（topbar 中段）============ */
.topbar-segmented {
  flex: none;
}
.topbar-segmented :deep(.seg-list) {
  height: 28px;
  padding: 2px;
  gap: 2px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.06);
  border: var(--hairline) solid var(--stroke-soft);
}
.dark .topbar-segmented :deep(.seg-list) {
  background: rgba(255, 255, 255, 0.05);
}
.topbar-segmented :deep(.seg-trigger) {
  height: 24px;
  padding: 0 10px;
  border-radius: 6px;
  font-size: var(--fs-callout);
  font-weight: 500;
  color: var(--fg-muted);
  background: transparent;
  transition: background-color 140ms ease-out, color 140ms ease-out, box-shadow 160ms ease-out;
}
.topbar-segmented :deep(.seg-trigger:hover) {
  color: var(--fg);
}
.topbar-segmented :deep(.seg-trigger[data-state='active']) {
  color: var(--fg-strong);
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 1.5px rgba(0, 0, 0, 0.06);
}
.dark .topbar-segmented :deep(.seg-trigger[data-state='active']) {
  box-shadow:
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.08),
    0 1px 1.5px rgba(0, 0, 0, 0.4);
}

/* ============ 内容 ============ */
.content-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* 检出弹窗：固定高度让内部 flex 布局（含任务输出）正常撑开 */
.checkout-dialog {
  width: 580px;
  max-width: calc(100vw - 48px);
  height: 620px;
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.checkout-dialog :deep(.checkout-view) {
  flex: 1;
  min-height: 0;
  padding: 4px 2px 0;
  background: transparent;
}
.tab-pane {
  flex: 1;
  height: 100%;
  min-height: 0;
  margin: 0;
  outline: none;
}
.tab-pane[data-state='inactive'] {
  display: none;
}
.empty-pane {
  padding: 64px 32px;
  text-align: center;
  color: var(--fg-muted);
  font-size: var(--fs-body);
}
.unavailable-pane {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}
.unavailable-pane p {
  margin: 0;
}
.unavailable-title {
  font-weight: 600;
  color: var(--fg);
}
.unavailable-path {
  font-size: var(--fs-mono);
  color: var(--fg-subtle);
  word-break: break-all;
}

/* ============ Statusbar ============ */
.statusbar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  padding: 0 12px;
  border-top: var(--hairline) solid var(--stroke);
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
  font-size: var(--fs-caption);
  color: var(--fg-muted);
}
.health-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.18);
}
.health-ok {
  background: var(--success);
  box-shadow:
    0 0 0 0.5px color-mix(in srgb, var(--success) 60%, transparent),
    0 0 6px color-mix(in srgb, var(--success) 45%, transparent);
}
.health-err {
  background: var(--danger);
  box-shadow:
    0 0 0 0.5px color-mix(in srgb, var(--danger) 60%, transparent),
    0 0 6px color-mix(in srgb, var(--danger) 50%, transparent);
}
.health-pending {
  background: var(--warning);
  animation: pulse 1.4s ease-in-out infinite;
}
.statusbar-text {
  font-size: var(--fs-caption);
}
.statusbar-err {
  color: var(--danger);
}
.statusbar-muted {
  color: var(--fg-muted);
}
.statusbar-spacer {
  flex: 1;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.45; }
}
</style>
