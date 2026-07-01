<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  ChevronDown,
  DownloadCloud,
  Folder,
  FolderGit2,
  FolderPlus,
  FolderSearch,
  HardDrive,
  Pencil,
  Plus,
  RefreshCw,
  ScrollText,
  SquareTerminal,
  Trash2,
} from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import ContextMenu, { type ContextMenuItem } from '@/components/ui-local/ContextMenu.vue'
import WorkingCopyChangesPopover from './WorkingCopyChangesPopover.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import { loadCollapsedNodes, saveCollapsedNodes } from '../lib/ui-state'
import { api } from '../api/svn'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useRepositoriesStore } from '../stores/repositories'
import { useTasksStore } from '../stores/tasks'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry, WorkingCopyStatusSummary } from '../types/svn'
import {
  getWorkingCopyLeafLabel,
  getWorkingCopyTreePath,
  getSmartSubtitle,
  getFullTitle as fullTitle,
  getDecodedBranchInfo,
  getDecodedUrl,
  type WorkingCopyTreeSegment,
} from '../lib/utils'

const props = withDefaults(defineProps<{ showActive?: boolean }>(), { showActive: true })

const emit = defineEmits<{
  select: [id: string]
  viewLog: [id: string]
}>()

const store = useWorkingCopiesStore()
const repoStore = useRepositoriesStore()
const tasksStore = useTasksStore()
const toast = useErrorToast()

const items = computed(() => store.items)
// 折叠的节点 key 从本地恢复，保持上次的展开/折叠形态
const collapsedNodes = ref<Set<string>>(new Set(loadCollapsedNodes()))

interface WorkingCopySummaryState extends WorkingCopyStatusSummary {
  loading: boolean
  error: boolean
}

const summaryById = ref<Record<string, WorkingCopySummaryState>>({})
let summaryGeneration = 0
const summaryRequestVersions = new Map<string, number>()
const summaryReloadTimers = new Map<string, ReturnType<typeof window.setTimeout>>()
const changesPopoverWc = ref<WorkingCopyEntry | null>(null)
const changesPopoverOpen = ref(false)
const changesPopoverX = ref(0)
const changesPopoverY = ref(0)

interface WorkingCopyTreeNode {
  key: string
  label: string
  value: string
  kind: WorkingCopyTreeSegment['kind']
  title?: string
  count: number
  children: WorkingCopyTreeNode[]
  copies: WorkingCopyEntry[]
}

type WorkingCopyTreeRow =
  | { kind: 'node'; key: string; node: WorkingCopyTreeNode; depth: number }
  | { kind: 'copy'; key: string; wc: WorkingCopyEntry; depth: number; fallbackLabel?: string }

function normalizeUrl(u: string) {
  return u.replace(/\/+$/, '')
}

// 若该远端 root 命中已添加的仓库配置（互为前缀视为同源），用用户起的仓库名做分组标题
function repoNameForRoot(root: string): string | null {
  const r = normalizeUrl(root)
  for (const repo of repoStore.items) {
    const u = normalizeUrl(repo.url)
    if (u === r || u.startsWith(`${r}/`) || r.startsWith(`${u}/`)) {
      return repo.name
    }
  }
  return null
}

const tree = computed(() => {
  const roots: WorkingCopyTreeNode[] = []
  for (const wc of store.items) {
    const segments = getWorkingCopyTreePath(wc).map(resolveTreeSegment)
    let level = roots
    let node: WorkingCopyTreeNode | null = null

    for (const segment of segments) {
      node = level.find((item) => item.key === segment.key) ?? null
      if (!node) {
        node = {
          key: segment.key,
          label: segment.label,
          value: segment.value,
          kind: segment.kind,
          title: segment.title,
          count: 0,
          children: [],
          copies: [],
        }
        level.push(node)
      }
      node.count += 1
      level = node.children
    }

    if (node) node.copies.push(wc)
  }

  sortTree(roots)
  return roots
})

const treeRows = computed(() => {
  const rows: WorkingCopyTreeRow[] = []

  function walk(node: WorkingCopyTreeNode, depth: number) {
    // 模块目录只有一个工作副本时，直接把模块行渲染为可操作叶子，避免 rest/rest 重复。
    if (node.kind === 'module' && node.children.length === 0 && node.copies.length === 1) {
      rows.push({
        kind: 'copy',
        key: `copy:${node.copies[0].id}`,
        wc: node.copies[0],
        depth,
        fallbackLabel: node.label,
      })
      return
    }

    rows.push({ kind: 'node', key: `node:${node.key}`, node, depth })
    if (collapsedNodes.value.has(node.key)) return

    for (const child of node.children) walk(child, depth + 1)
    for (const wc of node.copies) {
      rows.push({ kind: 'copy', key: `copy:${wc.id}`, wc, depth: depth + 1 })
    }
  }

  for (const root of tree.value) walk(root, 0)
  return rows
})

function resolveTreeSegment(segment: WorkingCopyTreeSegment): WorkingCopyTreeSegment {
  if (!segment.key.startsWith('repo:')) return segment
  const root = segment.key.slice(5)
  return {
    ...segment,
    label: repoNameForRoot(root) ?? rootLabel(root),
    title: getDecodedUrl(root),
  }
}

const ENV_ORDER = new Map([
  ['develop', 0],
  ['test', 1],
  ['produce', 2],
  ['默认', 3],
])
const MODULE_ORDER = new Map([
  ['front', 0],
  ['rest', 1],
  ['database', 2],
  ['updatesql', 3],
])

function sortTree(nodes: WorkingCopyTreeNode[]) {
  nodes.sort(compareTreeNode)
  for (const node of nodes) {
    sortTree(node.children)
    node.copies.sort((a, b) => copyLabel({ wc: a }).localeCompare(copyLabel({ wc: b }), 'zh-CN'))
  }
}

function compareTreeNode(a: WorkingCopyTreeNode, b: WorkingCopyTreeNode) {
  const rankA = treeNodeRank(a)
  const rankB = treeNodeRank(b)
  if (rankA !== rankB) return rankA - rankB
  return a.label.localeCompare(b.label, 'zh-CN')
}

function treeNodeRank(node: WorkingCopyTreeNode) {
  const lower = node.value.toLowerCase()
  if (node.kind === 'environment') return ENV_ORDER.get(lower) ?? 50
  if (node.kind === 'module') return MODULE_ORDER.get(lower) ?? 50
  return 0
}

function rowIndent(depth: number) {
  return `${8 + depth * 14}px`
}

function copyIndent(depth: number) {
  return `${6 + depth * 14}px`
}

function copyLabel(row: { wc: WorkingCopyEntry; fallbackLabel?: string }) {
  return row.wc.displayName || row.fallbackLabel || getWorkingCopyLeafLabel(row.wc)
}

onMounted(() => {
  store.reload().catch(toast)
})

watch(
  items,
  (list) => {
    void reloadSummaries(list)
  },
  { immediate: true },
)

// update、commit 或手动刷新完成后，只刷新 Store 指定的工作副本统计。
watch(
  () => store.statusRefreshRequest,
  (request) => {
    if (request) scheduleSummaryReload(request.id)
  },
)

// 任务 Store 显式发布完成信号，极快任务在注册时回放 finished 事件也不会漏刷新。
watch(
  () => tasksStore.completedTask,
  (task) => {
    if (
      task?.success &&
      task.workingCopyId &&
      (task.kind === 'update' || task.kind === 'commit')
    ) {
      scheduleSummaryReload(task.workingCopyId)
    }
  },
)

onUnmounted(() => {
  for (const timer of summaryReloadTimers.values()) window.clearTimeout(timer)
  summaryReloadTimers.clear()
})

function hasSummaryValue(summary: WorkingCopySummaryState | undefined) {
  return !!summary && (summary.loading || summary.error || summary.uncommitted > 0 || summary.outdated > 0)
}

function summaryTitle(summary: WorkingCopySummaryState | undefined) {
  if (!summary) return '正在统计工作副本状态'
  if (summary.error) return '状态统计失败，右键刷新后会重新读取'
  if (summary.loading) return '正在统计工作副本状态'
  return `未提交 ${summary.uncommitted}，未更新 ${summary.outdated}`
}

function openLocalChanges(event: MouseEvent, wc: WorkingCopyEntry) {
  const target = event.currentTarget as HTMLElement
  const rect = target.getBoundingClientRect()
  const sameCopy = changesPopoverWc.value?.id === wc.id
  if (sameCopy && changesPopoverOpen.value) {
    changesPopoverOpen.value = false
    return
  }
  changesPopoverWc.value = wc
  changesPopoverX.value = rect.right + 8
  changesPopoverY.value = rect.top - 8
  changesPopoverOpen.value = true
}

async function reloadSummaries(list: WorkingCopyEntry[]) {
  const generation = ++summaryGeneration
  const keepIds = new Set(list.map((wc) => wc.id))
  const next: Record<string, WorkingCopySummaryState> = {}
  for (const wc of list) {
    const old = summaryById.value[wc.id]
    next[wc.id] = old ?? { uncommitted: 0, outdated: 0, loading: wc.available !== false, error: false }
    if (wc.available === false) {
      next[wc.id] = { uncommitted: 0, outdated: 0, loading: false, error: false }
    }
  }
  summaryById.value = next

  const queue = list.filter((wc) => wc.available !== false && keepIds.has(wc.id))
  const concurrency = 3
  let cursor = 0

  async function worker() {
    while (cursor < queue.length && generation === summaryGeneration) {
      const wc = queue[cursor++]
      await reloadSummary(wc, () => generation === summaryGeneration)
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, worker))
}

function reloadSummaryById(id: string) {
  const wc = store.items.find((item) => item.id === id)
  if (!wc || wc.available === false) {
    return Promise.resolve()
  }
  return reloadSummary(wc)
}

// 合并同一副本紧邻的任务完成和 info 刷新通知，并给 SVN 元数据留出短暂稳定时间。
function scheduleSummaryReload(id: string) {
  const current = summaryReloadTimers.get(id)
  if (current) window.clearTimeout(current)
  const timer = window.setTimeout(() => {
    summaryReloadTimers.delete(id)
    void reloadSummaryById(id)
  }, 200)
  summaryReloadTimers.set(id, timer)
}

// 单副本请求独立编号，连续刷新时只接受最后一次返回结果。
async function reloadSummary(
  wc: WorkingCopyEntry,
  isCurrent: () => boolean = () => true,
) {
  const requestVersion = (summaryRequestVersions.get(wc.id) ?? 0) + 1
  summaryRequestVersions.set(wc.id, requestVersion)
  summaryById.value = {
    ...summaryById.value,
    [wc.id]: {
      ...(summaryById.value[wc.id] ?? { uncommitted: 0, outdated: 0 }),
      loading: true,
      error: false,
    },
  }
  try {
    const summary = await api.statusSummary(wc.path)
    if (!isCurrent() || summaryRequestVersions.get(wc.id) !== requestVersion) return
    summaryById.value = {
      ...summaryById.value,
      [wc.id]: { ...summary, loading: false, error: false },
    }
  } catch {
    if (!isCurrent() || summaryRequestVersions.get(wc.id) !== requestVersion) return
    summaryById.value = {
      ...summaryById.value,
      [wc.id]: {
        ...(summaryById.value[wc.id] ?? { uncommitted: 0, outdated: 0 }),
        loading: false,
        error: true,
      },
    }
  }
}

async function pickAndAdd() {
  try {
    const dir = await open({ directory: true, multiple: false, title: '选择 SVN 工作副本目录' })
    if (!dir || typeof dir !== 'string') return
    await store.add(dir)
    void reloadSummaries(store.items)
  } catch (e) {
    toast(e, '添加工作副本失败')
  }
}

// 选一个项目根目录，自动识别其下 develop/test/produce × rest/database/updatesql 等所有分支模块
async function pickAndScanProject() {
  try {
    const dir = await open({ directory: true, multiple: false, title: '选择项目根目录（自动识别所有分支）' })
    if (!dir || typeof dir !== 'string') return
    const touched = await store.scanProject(dir)
    if (touched.length === 0) {
      toast('未在该目录下识别到工作副本，请确认选的是项目根目录', '没有识别到分支')
    }
  } catch (e) {
    toast(e, '扫描项目失败')
  }
}

async function refresh(id: string) {
  try {
    await store.refresh(id)
  } catch (e) {
    toast(e, '刷新失败')
  }
}

async function remove(id: string) {
  const ok = await confirm({
    title: '移除工作副本',
    content: '移除这个工作副本？只是从列表里去掉，不会删除磁盘文件。',
    confirmText: '移除',
    destructive: true,
  })
  if (!ok) return
  try {
    await store.remove(id)
  } catch (e) {
    toast(e, '移除失败')
  }
}

function rootLabel(root: string) {
  try {
    const u = new URL(root)
    return u.pathname.split('/').filter(Boolean).pop() || u.host
  } catch {
    const parts = root.split(/[\\/]/).filter(Boolean)
    return parts[parts.length - 1] || root
  }
}

function selectWc(id: string) {
  store.select(id)
  emit('select', id)
}

function firstCopyForNode(node: WorkingCopyTreeNode): WorkingCopyEntry | null {
  const available = node.copies.find((wc) => wc.available !== false)
  if (available) return available
  if (node.copies[0]) return node.copies[0]
  for (const child of node.children) {
    const wc = firstCopyForNode(child)
    if (wc) return wc
  }
  return null
}

// 点击项目/分支目录时切换到该目录下的代表工作副本，避免只能点叶子路径。
function selectNode(node: WorkingCopyTreeNode) {
  const wc = firstCopyForNode(node)
  if (wc) selectWc(wc.id)
}

function nodeHasSelected(node: WorkingCopyTreeNode): boolean {
  const selectedId = store.selectedId
  if (!selectedId) return false
  if (node.copies.some((wc) => wc.id === selectedId)) return true
  return node.children.some(nodeHasSelected)
}

function toggleNode(key: string) {
  const next = new Set(collapsedNodes.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  collapsedNodes.value = next
  saveCollapsedNodes([...next])
}

// ===== 右键菜单 =====
const ctxOpen = ref(false)
const ctxX = ref(0)
const ctxY = ref(0)
const ctxWc = ref<WorkingCopyEntry | null>(null)

const ctxItems: ContextMenuItem[] = [
  { key: 'log', label: '查看日志', icon: ScrollText },
  { key: 'refresh', label: '重新读取 svn info', icon: RefreshCw },
  { key: 'edit', label: '编辑显示名称', icon: Pencil },
  { key: 'sep1', separator: true, label: '' },
  { key: 'reveal', label: '在访达中打开', icon: FolderSearch },
  { key: 'terminal', label: '在终端中打开', icon: SquareTerminal },
  { key: 'sep2', separator: true, label: '' },
  { key: 'update', label: '从远程更新到本地', icon: DownloadCloud },
  { key: 'remove', label: '从列表移除', icon: Trash2, danger: true },
]

function openContextMenu(event: MouseEvent, wc: WorkingCopyEntry) {
  selectWc(wc.id)
  ctxWc.value = wc
  ctxX.value = event.clientX
  ctxY.value = event.clientY
  ctxOpen.value = true
}

async function onCtxSelect(key: string) {
  const wc = ctxWc.value
  if (!wc) return
  switch (key) {
    case 'log':
      emit('viewLog', wc.id)
      break
    case 'refresh':
      await refresh(wc.id)
      break
    case 'edit':
      await startEdit(wc)
      break
    case 'reveal':
      try {
        await api.revealInFileManager(wc.path)
      } catch (e) {
        toast(e, '在访达中打开失败')
      }
      break
    case 'terminal':
      try {
        await api.openInTerminal(wc.path)
      } catch (e) {
        toast(e, '在终端中打开失败')
      }
      break
    case 'update':
      try {
        await launchUpdate(wc)
      } catch (e) {
        toast(e, '启动更新失败')
      }
      break
    case 'remove':
      await remove(wc.id)
      break
  }
}

// 从右键菜单直接发起整副本更新，进度走统一任务面板，返回任务 id 供重试
async function launchUpdate(wc: WorkingCopyEntry): Promise<string> {
  const id = await api.startUpdate(wc.path)
  const name = wc.displayName || getWorkingCopyLeafLabel(wc)
  tasksStore.register({
    taskId: id,
    kind: 'update',
    workingCopyId: wc.id,
    title: `更新 ${name} 到 HEAD`,
    command: `svn update ${wc.path}`,
    retry: () => launchUpdate(wc),
  })
  // 右键更新不经过 StatusView，成功后主动刷新工作副本 revision，保证历史页签标记不滞后。
  const refreshWhenFinished = async () => {
    const task = tasksStore.tasks.get(id)
    if (task?.success) await refresh(wc.id)
  }
  let stop: (() => void) | null = null
  stop = watch(
    () => tasksStore.tasks.get(id)?.finished,
    async (finished) => {
      if (!finished) return
      stop?.()
      await refreshWhenFinished()
    },
  )
  if (tasksStore.tasks.get(id)?.finished) {
    stop?.()
    await refreshWhenFinished()
  }
  // 右键更新没有独立输出区，自动弹开任务中心让用户看到进度与结果
  tasksStore.openCenter()
  return id
}

// 内联编辑状态：彻底避开 prompt + tooltip + hover 的事件竞争问题
const editingId = ref<string | null>(null)
const editingValue = ref('')
const editingInputRef = ref<any>(null)

async function startEdit(wc: WorkingCopyEntry, fallbackLabel?: string) {
  editingId.value = wc.id
  editingValue.value = wc.displayName || fallbackLabel || getWorkingCopyLeafLabel(wc)
  await nextTick()
  // 聚焦原生 input（Input 组件根元素即 input）
  try {
    const el = editingInputRef.value?.$el || editingInputRef.value
    const input = el?.focus ? el : el?.querySelector?.('input')
    if (input) {
      input.focus()
      if (input.select) input.select()
    }
  } catch {}
}

async function commitEdit() {
  const id = editingId.value
  if (!id) return
  const val = editingValue.value.trim()
  const newName = val ? val : null
  try {
    await store.setDisplayName(id, newName)
  } catch (e) {
    toast(e, '保存显示名称失败')
  }
  editingId.value = null
  editingValue.value = ''
}

function cancelEdit() {
  editingId.value = null
  editingValue.value = ''
}
</script>

<template>
  <div class="wc-list">
    <div class="wc-section-head">
      <span class="section-title">工作副本</span>
      <div class="head-actions">
        <Button size="xs" variant="ghost" class="head-action" title="选项目根目录，自动识别所有分支模块" @click="pickAndScanProject">
          <FolderPlus class="icon-xs" />
          项目
        </Button>
        <Button size="xs" variant="ghost" class="head-action" title="添加单个工作副本目录" @click="pickAndAdd">
          <Plus class="icon-xs" />
          添加
        </Button>
      </div>
    </div>
    <div class="wc-scroll">
      <div v-if="items.length === 0" class="wc-empty">
        <EmptyState description="还没有工作副本，点上面按钮选一个本地目录" />
      </div>
      <template v-for="row in treeRows" :key="row.key">
        <button
          v-if="row.kind === 'node'"
          :class="['root-row', 'tree-row', { 'child-row': row.depth > 0, active: nodeHasSelected(row.node) }]"
          :style="{ paddingLeft: rowIndent(row.depth) }"
          type="button"
          @click="selectNode(row.node)"
        >
          <ChevronDown
            :class="['root-chevron', { collapsed: collapsedNodes.has(row.node.key) }]"
            @click.stop="toggleNode(row.node.key)"
          />
          <component
            :is="row.depth === 0 ? FolderGit2 : Folder"
            :class="['root-icon', { 'child-icon': row.depth > 0 }]"
          />
          <span
            class="root-name"
            :title="row.node.title || row.node.label"
          >{{ row.node.label }}</span>
          <span class="root-count mono">{{ row.node.count }}</span>
        </button>
        <div
          v-else
          :class="[
            'wc-item',
            {
              active: props.showActive && row.wc.id === store.selectedId,
              unavailable: row.wc.available === false,
            },
          ]"
          :style="{ marginLeft: copyIndent(row.depth) }"
          :title="row.wc.available === false ? '路径不可用：磁盘卷可能未挂载' : undefined"
          @click="selectWc(row.wc.id)"
          @contextmenu.prevent="openContextMenu($event, row.wc)"
        >
          <div class="wc-row-main">
            <HardDrive class="wc-icon" />
            <div class="wc-text">
              <div class="wc-name-wrap">
                <template v-if="editingId === row.wc.id">
                  <Input
                    ref="editingInputRef"
                    v-model="editingValue"
                    class="edit-name-input"
                    @click.stop
                    @keydown.enter="commitEdit"
                    @keydown.esc="cancelEdit"
                    @blur="commitEdit"
                  />
                </template>
                <div
                  v-else
                  class="wc-name"
                  :title="fullTitle(row.wc)"
                >
                  {{ copyLabel(row) }}
                </div>
              </div>
              <div v-if="editingId !== row.wc.id" class="wc-meta">
                <span v-if="row.wc.revision" class="meta-rev mono">r{{ row.wc.revision }}</span>
                <span
                  v-if="getSmartSubtitle(row.wc)"
                  class="wc-url mono"
                  :title="getDecodedBranchInfo(row.wc) || ''"
                >{{ getSmartSubtitle(row.wc) }}</span>
              </div>
            </div>
            <Transition name="summary-pop">
              <div
                v-if="hasSummaryValue(summaryById[row.wc.id])"
                :class="[
                  'wc-summary',
                  {
                    loading: summaryById[row.wc.id]?.loading,
                    error: summaryById[row.wc.id]?.error,
                  },
                ]"
                :title="summaryTitle(summaryById[row.wc.id])"
              >
                <button
                  v-if="summaryById[row.wc.id]?.uncommitted || summaryById[row.wc.id]?.loading"
                  type="button"
                  :class="[
                    'summary-chip',
                    'summary-local',
                    'summary-local-button',
                    { skeleton: summaryById[row.wc.id]?.loading },
                  ]"
                  :disabled="summaryById[row.wc.id]?.loading"
                  :aria-label="`查看 ${summaryById[row.wc.id]?.uncommitted || 0} 个本地更改文件`"
                  title="查看本地更改文件"
                  data-local-changes-trigger
                  @click.stop="openLocalChanges($event, row.wc)"
                >
                  <span class="summary-number mono">
                    {{ summaryById[row.wc.id]?.loading ? '' : summaryById[row.wc.id]?.uncommitted }}
                  </span>
                </button>
                <span
                  v-if="summaryById[row.wc.id]?.outdated || summaryById[row.wc.id]?.loading"
                  :class="['summary-chip', 'summary-remote', { skeleton: summaryById[row.wc.id]?.loading }]"
                >
                  <span class="summary-number mono">
                    {{ summaryById[row.wc.id]?.loading ? '' : summaryById[row.wc.id]?.outdated }}
                  </span>
                </span>
                <span v-if="summaryById[row.wc.id]?.error" class="summary-chip summary-error mono">!</span>
              </div>
            </Transition>
          </div>
        </div>
      </template>
    </div>

    <ContextMenu
      v-model:open="ctxOpen"
      :x="ctxX"
      :y="ctxY"
      :items="ctxItems"
      @select="onCtxSelect"
    />
    <WorkingCopyChangesPopover
      v-model:open="changesPopoverOpen"
      :x="changesPopoverX"
      :y="changesPopoverY"
      :working-copy="changesPopoverWc"
    />
  </div>
</template>

<style scoped>
.wc-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: transparent;
}

/* ===== 分组标题（NSSourceList 风格：小写灰、字距）===== */
.wc-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 14px 6px 14px;
  user-select: none;
}
.section-title {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--fg-muted);
}
.head-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}
.head-action {
  height: 22px;
  padding: 0 6px;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  gap: 4px;
}
.head-action:hover {
  color: var(--fg-strong);
}

.wc-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow: auto;
  padding: 2px 0 8px;
}
.wc-empty {
  padding: 16px 12px;
  text-align: center;
}

/* ===== 工作副本树 ===== */
.root-row {
  width: calc(100% - 12px);
  margin: 0 6px 1px;
  min-height: 26px;
  border: 0;
  border-radius: var(--radius-row);
  background: transparent;
  color: var(--fg);
  display: grid;
  grid-template-columns: 14px 16px minmax(0, 1fr) auto;
  gap: 7px;
  align-items: center;
  cursor: pointer;
  text-align: left;
  font: inherit;
  padding: 3px 8px;
  transition: background-color 120ms ease-out;
}
.root-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.root-row.active {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.tree-row.child-row {
  min-height: 24px;
}
.tree-row.child-row .root-name {
  font-weight: 500;
  color: var(--fg);
}
.root-chevron {
  width: 12px;
  height: 12px;
  color: var(--fg-muted);
  transition: transform 140ms ease-out;
}
.root-chevron.collapsed {
  transform: rotate(-90deg);
}
.root-icon {
  width: 14px;
  height: 14px;
  color: var(--accent);
  flex: none;
}
.root-icon.child-icon {
  color: var(--folder);
  opacity: 0.88;
}
.root-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
  color: var(--fg-strong);
  font-size: var(--fs-callout);
}
.root-count {
  color: var(--fg-subtle);
  font-size: var(--fs-caption);
  font-feature-settings: 'tnum';
}

/* ===== 工作副本行（NSSourceList capsule selection）===== */
.wc-item {
  margin: 1px 6px;
  padding: 0;
  border-radius: var(--radius-row);
  background: transparent;
  cursor: pointer;
  transition: background-color 120ms ease-out;
  position: relative;
}
.wc-item:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.wc-item.active {
  background: var(--accent);
  color: var(--fg-on-accent);
}
/* 路径不可用（卷未挂载/目录被删）：整行降饱和置灰，仍可点击查看提示 */
.wc-item.unavailable {
  opacity: 0.45;
  filter: grayscale(0.6);
}
.wc-row-main {
  position: relative;
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  min-height: 36px;
}
.wc-icon {
  width: 14px;
  height: 14px;
  color: var(--fg-muted);
  flex: none;
}
.wc-item.active .wc-icon {
  color: rgba(255, 255, 255, 0.85);
}
.wc-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.wc-name {
  font-size: var(--fs-callout);
  font-weight: 500;
  color: var(--fg-strong);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-item.active .wc-name {
  color: #fff;
  font-weight: 600;
}
.wc-meta {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}
.wc-item.active .wc-meta {
  color: rgba(255, 255, 255, 0.75);
}
.meta-rev {
  flex: none;
  font-feature-settings: 'tnum';
}
.wc-url {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-summary {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0;
  min-width: 0;
  max-width: 82px;
  height: 20px;
  padding: 1px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--mat-elevated) 58%, transparent);
  backdrop-filter: blur(16px) saturate(150%);
  box-shadow:
    inset 0 0 0 0.5px color-mix(in srgb, var(--fg) 14%, transparent),
    inset 0 1px 0 rgba(255, 255, 255, 0.45),
    0 1px 2px rgba(15, 23, 42, 0.05);
  overflow: hidden;
}
.wc-summary::before {
  content: '';
  position: absolute;
  inset: 1px;
  border-radius: inherit;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.42), transparent 55%);
  opacity: 0.7;
  pointer-events: none;
}
.wc-summary::after {
  content: '';
  position: absolute;
  inset: 2px;
  border-radius: inherit;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.36), transparent);
  opacity: 0;
  pointer-events: none;
  transform: translateX(-115%);
}
.wc-summary.loading {
  background: color-mix(in srgb, var(--fg) 5%, transparent);
}
.wc-summary.loading::after {
  opacity: 1;
  animation: summary-sheen 1.35s ease-in-out infinite;
}
.wc-summary.error {
  background: color-mix(in srgb, var(--danger) 10%, var(--mat-elevated) 70%);
  box-shadow:
    inset 0 0 0 0.5px color-mix(in srgb, var(--danger) 34%, transparent),
    0 1px 2px rgba(15, 23, 42, 0.05);
}
.summary-chip {
  position: relative;
  z-index: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 18px;
  padding: 0 5px;
  border-radius: 0;
  font-size: 10px;
  font-weight: 650;
  line-height: 1;
  font-feature-settings: 'tnum';
  white-space: nowrap;
  transition:
    background-color 140ms ease-out,
    color 140ms ease-out,
    transform 140ms ease-out;
}
.summary-chip:first-child {
  border-radius: var(--radius-pill) 0 0 var(--radius-pill);
}
.summary-chip:last-child {
  border-radius: 0 var(--radius-pill) var(--radius-pill) 0;
}
.summary-chip:only-child {
  border-radius: var(--radius-pill);
}
.summary-chip + .summary-chip {
  box-shadow: inset 1px 0 0 color-mix(in srgb, var(--fg) 13%, transparent);
}
.summary-local {
  color: color-mix(in srgb, var(--warning) 64%, var(--fg-strong));
  background: color-mix(in srgb, var(--warning) 13%, transparent);
}
.summary-local-button {
  border: 0;
  font-family: inherit;
  cursor: pointer;
}
.summary-local-button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--warning) 22%, transparent);
}
.summary-local-button:active:not(:disabled) {
  transform: scale(0.94);
}
.summary-local-button:focus-visible {
  outline: 2px solid var(--accent-ring);
  outline-offset: -1px;
}
.summary-local-button:disabled {
  cursor: default;
}
.summary-remote {
  color: color-mix(in srgb, var(--accent) 74%, var(--fg-strong));
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.summary-number {
  min-width: 6px;
  text-align: center;
  letter-spacing: 0;
}
.summary-chip.skeleton {
  width: 26px;
  color: transparent;
  background: color-mix(in srgb, var(--fg) 7%, transparent);
}
.summary-chip.skeleton .summary-number {
  opacity: 0;
}
.summary-error {
  min-width: 16px;
  height: 18px;
  color: #fff;
  background: var(--danger);
}
.wc-item.active .wc-summary {
  background: rgba(255, 255, 255, 0.16);
  box-shadow:
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.26),
    inset 0 1px 0 rgba(255, 255, 255, 0.18);
}
.wc-item.active .summary-local {
  color: #fff;
  background: rgba(255, 255, 255, 0.18);
}
.wc-item.active .summary-local-button:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.3);
}
.wc-item.active .summary-remote {
  color: #fff;
  background: rgba(255, 255, 255, 0.26);
}
.summary-pop-enter-active,
.summary-pop-leave-active {
  transition:
    opacity 150ms ease-out,
    transform 170ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.summary-pop-enter-from,
.summary-pop-leave-to {
  opacity: 0;
  transform: translateX(5px) scale(0.94);
}
@keyframes summary-sheen {
  0% {
    transform: translateX(-115%);
  }
  100% {
    transform: translateX(115%);
  }
}
.icon-xs {
  width: 12px;
  height: 12px;
}

/* ===== 内联编辑输入（替换名称行，适配列表密度）===== */
.wc-name-wrap {
  min-width: 0;
}
.edit-name-input {
  height: 24px;
  font-size: var(--fs-callout);
  font-weight: 500;
  /* 让输入框在行内不被绝对定位的 actions 遮挡，actions 编辑时已隐藏 */
  padding-right: 4px;
}
.wc-item.active .edit-name-input {
  color: #fff;
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.3);
}
</style>
