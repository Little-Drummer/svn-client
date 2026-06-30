<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronRight,
  Copy,
  Eye,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  FolderSearch,
  Plus,
  RefreshCw,
  Trash2,
  Undo2,
  EyeOff,
  X,
} from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import ContextMenu, { type ContextMenuItem } from '@/components/ui-local/ContextMenu.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import DiffViewer from '../components/DiffViewer.vue'
import CommitPanel from '../components/CommitPanel.vue'
import UpdatePanel from '../components/UpdatePanel.vue'

import { api } from '../api/svn'
import { useStatusStore } from '../stores/status'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useErrorToast } from '../composables/use-error-toast'
import { createGeneration } from '../composables/use-request-generation'
import type { SvnStatusEntry, WorkingCopyEntry, WorkingCopyFileEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry }>()

const statusStore = useStatusStore()
const wcStore = useWorkingCopiesStore()
const toast = useErrorToast()

const selectedFile = ref<SvnStatusEntry | null>(null)
const checkedPaths = ref<Set<string>>(new Set())
const diffText = ref<string | null>(null)
const baseContent = ref<string | null>(null)
const currentContent = ref<string | null>(null)
const diffLoading = ref(false)
// 右侧栏按需出现：默认只看文件列表，选中文件才滑出 diff，点提交/更新才滑出对应面板
const rightPane = ref<'none' | 'diff' | 'commit' | 'update'>('none')
const leftMode = ref<'tree' | 'changes'>('tree')
const fileTree = ref<WorkingCopyFileEntry[]>([])
const fileTreeLoading = ref(false)
const expandedDirs = ref<Set<string>>(new Set())
const selectedTreePath = ref<string | null>(null)
const showCreateFolder = ref(false)
const folderName = ref('')

// 状态分组顺序
const STATUS_ORDER = [
  'conflicted',
  'modified',
  'added',
  'deleted',
  'replaced',
  'missing',
  'obstructed',
  'unversioned',
  'ignored',
  'incomplete',
  'external',
  'normal',
]
const STATUS_LABEL: Record<string, string> = {
  modified: '已修改',
  added: '新增',
  deleted: '删除',
  replaced: '替换',
  missing: '丢失',
  conflicted: '冲突',
  unversioned: '未跟踪',
  ignored: '已忽略',
  obstructed: '阻塞',
  external: '外部',
  incomplete: '未完成',
  normal: '正常',
}
const STATUS_CLASSES: Record<string, string> = {
  modified: 'status-modified',
  added: 'status-added',
  deleted: 'status-deleted',
  replaced: 'status-warning',
  conflicted: 'status-deleted',
  missing: 'status-warning',
  obstructed: 'status-warning',
  unversioned: 'status-muted',
  ignored: 'status-muted',
  external: 'status-muted',
  incomplete: 'status-warning',
  normal: 'status-muted',
}

type FileSortKey = 'name' | 'modifiedAt' | 'size' | 'kind' | 'revision' | 'status' | 'author'
type SortDirection = 'asc' | 'desc'

const FILE_COLUMNS: { key: FileSortKey; label: string; className?: string }[] = [
  { key: 'name', label: '名称', className: 'col-name' },
  { key: 'modifiedAt', label: '修改时间', className: 'col-date' },
  { key: 'size', label: '大小', className: 'col-size' },
  { key: 'kind', label: '类型', className: 'col-kind' },
  { key: 'revision', label: '修订版本', className: 'col-rev' },
  { key: 'status', label: '状态', className: 'col-status' },
  { key: 'author', label: '作者', className: 'col-author' },
]

const sortState = ref<{ key: FileSortKey; direction: SortDirection }>({
  key: 'name',
  direction: 'asc',
})

function toggleSort(key: FileSortKey) {
  if (sortState.value.key === key) {
    sortState.value = {
      key,
      direction: sortState.value.direction === 'asc' ? 'desc' : 'asc',
    }
    return
  }
  sortState.value = { key, direction: key === 'modifiedAt' || key === 'revision' ? 'desc' : 'asc' }
}

function sortIcon(key: FileSortKey) {
  if (sortState.value.key !== key) return ArrowUpDown
  return sortState.value.direction === 'asc' ? ArrowUp : ArrowDown
}

// 提交面板可选择的状态；未跟踪文件会在提交前自动执行 svn add。
const COMMITTABLE = new Set([
  'modified',
  'added',
  'deleted',
  'replaced',
  'conflicted',
  'unversioned',
])

const checkedCommittablePaths = computed(() =>
  [...checkedPaths.value].filter((path) => {
    const entry = statusStore.entries.find((e) => e.path === path)
    return entry ? COMMITTABLE.has(entry.item) : false
  }),
)
const checkedUnversionedPaths = computed(() =>
  checkedCommittablePaths.value.filter(
    (path) => statusStore.entries.find((entry) => entry.path === path)?.item === 'unversioned',
  ),
)
const statusByPath = computed(() => {
  const map = new Map<string, SvnStatusEntry>()
  for (const entry of statusStore.entries) map.set(entry.path, entry)
  return map
})

const fileTreeByPath = computed(() => {
  const map = new Map<string, WorkingCopyFileEntry>()
  function visit(entries: WorkingCopyFileEntry[]) {
    for (const entry of entries) {
      map.set(entry.path, entry)
      visit(entry.children)
    }
  }
  visit(fileTree.value)
  return map
})

function normalizeComparePath(path: string) {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

const ignoredRootPaths = computed(() => {
  const roots: string[] = []
  function visit(entries: WorkingCopyFileEntry[]) {
    for (const entry of entries) {
      if (entry.svnItem === 'ignored') {
        roots.push(normalizeComparePath(entry.path))
        continue
      }
      visit(entry.children)
    }
  }
  visit(fileTree.value)
  return roots
})

function isUnderIgnoredRoot(path: string) {
  const normalized = normalizeComparePath(path)
  return ignoredRootPaths.value.some((root) => normalized === root || normalized.startsWith(`${root}/`))
}

function rawFileStatus(path: string) {
  if (isUnderIgnoredRoot(path)) return 'ignored'
  return statusByPath.value.get(path)?.item ?? fileTreeByPath.value.get(path)?.svnItem ?? 'normal'
}

function fileRevision(entry: WorkingCopyFileEntry) {
  const status = statusByPath.value.get(entry.path)
  return status?.commitRevision ?? status?.revision ?? entry.commitRevision ?? entry.revision ?? null
}

function fileAuthor(entry: WorkingCopyFileEntry) {
  const status = statusByPath.value.get(entry.path)
  return status?.commitAuthor ?? entry.commitAuthor ?? ''
}

function fileDate(entry: WorkingCopyFileEntry) {
  return entry.modifiedAt ?? entry.commitDate ?? ''
}

function fileKindLabel(entry: WorkingCopyFileEntry) {
  if (entry.kind === 'dir') return '文件夹'
  const ext = entry.name.split('.').pop()?.toLowerCase() ?? ''
  const map: Record<string, string> = {
    ts: 'TypeScript 文件',
    tsx: 'TypeScript 文件',
    vue: 'Vue 文件',
    js: 'JavaScript 文件',
    jsx: 'JavaScript 文件',
    json: 'JSON 文件',
    md: 'Markdown 文件',
    rs: 'Rust 文件',
    css: '样式表',
    scss: 'SCSS 样式表',
    html: 'HTML 文件',
    lock: '锁定文件',
    txt: '文本文件',
    png: '图片',
    jpg: '图片',
    jpeg: '图片',
    gif: '图片',
    svg: '矢量图片',
  }
  if (entry.name.endsWith('.lock')) return '锁定文件'
  return map[ext] ?? '文档'
}

function formatFileSize(size?: number | null, kind?: string) {
  if (kind === 'dir') return '--'
  if (size == null) return '--'
  if (size < 1024) return `${size} 字节`
  if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`
  return `${(size / 1024 / 1024).toFixed(size < 10 * 1024 * 1024 ? 1 : 0)} MB`
}

function formatFileDate(value?: string | null) {
  if (!value) return '--'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '--'
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function changeFileEntry(entry: SvnStatusEntry) {
  return fileTreeByPath.value.get(entry.path)
}

function changeRevision(entry: SvnStatusEntry) {
  const file = changeFileEntry(entry)
  return entry.commitRevision ?? entry.revision ?? file?.commitRevision ?? file?.revision ?? null
}

function changeAuthor(entry: SvnStatusEntry) {
  return entry.commitAuthor ?? changeFileEntry(entry)?.commitAuthor ?? ''
}

function changeModifiedAt(entry: SvnStatusEntry) {
  return changeFileEntry(entry)?.modifiedAt ?? entry.commitDate ?? ''
}

function changeSize(entry: SvnStatusEntry) {
  return changeFileEntry(entry)?.size ?? null
}

function changeKindLabel(entry: SvnStatusEntry) {
  const file = changeFileEntry(entry)
  if (file) return fileKindLabel(file)
  return shortName(entry.path).includes('.') ? '文档' : '项目'
}

function statusRank(item: string) {
  const index = STATUS_ORDER.indexOf(item)
  return index >= 0 ? index : STATUS_ORDER.length
}

function compareValues(a: string | number | null | undefined, b: string | number | null | undefined) {
  if (typeof a === 'number' && typeof b === 'number') return a - b
  return String(a).localeCompare(String(b), undefined, { sensitivity: 'base', numeric: true })
}

function fileSortValue(entry: WorkingCopyFileEntry, key: FileSortKey): string | number | null {
  switch (key) {
    case 'name':
      return entry.name
    case 'modifiedAt':
      return Date.parse(fileDate(entry)) || null
    case 'size':
      return entry.kind === 'dir' ? null : (entry.size ?? null)
    case 'kind':
      return fileKindLabel(entry)
    case 'revision':
      return fileRevision(entry)
    case 'status':
      return statusRank(fileStatus(entry.path))
    case 'author':
      return fileAuthor(entry)
  }
}

function changeSortValue(entry: SvnStatusEntry, key: FileSortKey): string | number | null {
  switch (key) {
    case 'name':
      return shortName(entry.path)
    case 'modifiedAt':
      return Date.parse(changeModifiedAt(entry)) || null
    case 'size':
      return changeSize(entry)
    case 'kind':
      return changeKindLabel(entry)
    case 'revision':
      return changeRevision(entry)
    case 'status':
      return statusRank(entry.item)
    case 'author':
      return changeAuthor(entry)
  }
}

function compareBySort<T>(
  a: T,
  b: T,
  getValue: (item: T, key: FileSortKey) => string | number | null,
  getName: (item: T) => string,
) {
  const { key, direction } = sortState.value
  const aValue = getValue(a, key)
  const bValue = getValue(b, key)
  const aEmpty = aValue == null || aValue === ''
  const bEmpty = bValue == null || bValue === ''
  if (aEmpty && bEmpty) return getName(a).localeCompare(getName(b), undefined, { sensitivity: 'base', numeric: true })
  if (aEmpty && !bEmpty) return 1
  if (!aEmpty && bEmpty) return -1
  const result = compareValues(aValue, bValue)
  if (result !== 0) return direction === 'asc' ? result : -result
  return getName(a).localeCompare(getName(b), undefined, { sensitivity: 'base', numeric: true })
}

const sortedFileTree = computed(() => {
  // 排序只作用于同级节点，避免破坏目录层级和展开状态。
  function sortLevel(entries: WorkingCopyFileEntry[]): WorkingCopyFileEntry[] {
    return entries
      .map((entry) => ({ ...entry, children: sortLevel(entry.children) }))
      .sort((a, b) => compareBySort(a, b, fileSortValue, (entry) => entry.name))
  }
  return sortLevel(fileTree.value)
})

const flatFileTree = computed(() => {
  const rows: { entry: WorkingCopyFileEntry; depth: number }[] = []
  function visit(entries: WorkingCopyFileEntry[], depth: number) {
    for (const entry of entries) {
      const status = rawFileStatus(entry.path)
      if (!statusStore.showUnversioned && status === 'unversioned') {
        continue
      }
      if (!statusStore.showIgnored && status === 'ignored') {
        continue
      }
      rows.push({ entry, depth })
      if (entry.kind === 'dir' && expandedDirs.value.has(entry.path)) {
        visit(entry.children, depth + 1)
      }
    }
  }
  visit(sortedFileTree.value, 0)
  return rows
})

type ChangeRow = { kind: 'entry'; entry: SvnStatusEntry }

const flatChanges = computed<ChangeRow[]>(() => {
  return [...statusStore.entries]
    .filter((entry) => statusStore.showUnversioned || entry.item !== 'unversioned')
    .filter((entry) => statusStore.showIgnored || entry.item !== 'ignored')
    .sort((a, b) => compareBySort(a, b, changeSortValue, (entry) => shortName(entry.path)))
    .map((entry) => ({ kind: 'entry', entry }))
})

const visibleCommittable = computed(() =>
  flatChanges.value.map((row) => row.entry).filter((e) => COMMITTABLE.has(e.item)),
)
const allChecked = computed(
  () =>
    visibleCommittable.value.length > 0 &&
    visibleCommittable.value.every((e) => checkedPaths.value.has(e.path)),
)
const visibleCheckedCount = computed(
  () => flatChanges.value.filter((row) => checkedPaths.value.has(row.entry.path)).length,
)

// ====== 虚拟滚动器 ======
// useVirtualizer 返回 Ref<Virtualizer>；options 用 computed 包裹以响应式追踪 count
const treeScrollRef = ref<HTMLElement | null>(null)
const changesScrollRef = ref<HTMLElement | null>(null)

const treeVirtualizer = useVirtualizer(
  computed(() => {
    // 显式读取 ref 让 computed 跟踪挂载/卸载，避免 leftMode 切换后 virtualizer 不重新订阅
    const el = treeScrollRef.value
    return {
      count: flatFileTree.value.length,
      getScrollElement: () => el,
      estimateSize: () => 28,
      overscan: 10,
    }
  }),
)

const changesVirtualizer = useVirtualizer(
  computed(() => {
    const el = changesScrollRef.value
    return {
      count: flatChanges.value.length,
      getScrollElement: () => el,
      estimateSize: () => 30,
      overscan: 10,
    }
  }),
)

function toggleAll(v: boolean) {
  if (v) {
    for (const e of flatChanges.value) {
      if (COMMITTABLE.has(e.entry.item)) checkedPaths.value.add(e.entry.path)
    }
  } else {
    for (const e of flatChanges.value) {
      if (COMMITTABLE.has(e.entry.item)) checkedPaths.value.delete(e.entry.path)
    }
  }
}

function toggleEntry(e: SvnStatusEntry, v: boolean) {
  if (v) checkedPaths.value.add(e.path)
  else checkedPaths.value.delete(e.path)
}

// 切 WC 时待恢复的选中文件路径——reload 完成后再去 entries 里找回实体
let pendingSelectedFilePath: string | null = null

async function reload() {
  // 流式刷新：entry 分批到达即渲染；依赖完整 entries 的清理在流结束后由 watch 处理
  await statusStore.reloadStreaming(props.workingCopy.path)
  await reloadFileTree()
}

async function reloadAfterWorkingCopyChange() {
  await wcStore.refresh(props.workingCopy.id)
  await reload()
}

// 显式先写值再刷新，避免 v-model 与副作用监听器的执行顺序导致用旧 flag 拉取
function onToggleUnversioned(v: boolean | 'indeterminate') {
  statusStore.showUnversioned = v === true
  if (!statusStore.showUnversioned) {
    // 隐藏未跟踪时同步清掉相关勾选，避免工具栏保留不可见文件的批量操作状态。
    for (const entry of statusStore.entries) {
      if (entry.item === 'unversioned') {
        checkedPaths.value.delete(entry.path)
      }
    }
  }
  reload().catch(toast)
}

function onToggleIgnored() {
  statusStore.showIgnored = !statusStore.showIgnored
  if (!statusStore.showIgnored) {
    for (const entry of statusStore.entries) {
      if (entry.item === 'ignored') checkedPaths.value.delete(entry.path)
    }
  }
  reload().catch(toast)
}

// 流式刷新完成（loading 由 true 变 false）后，基于完整 entries 做勾选清理与选中恢复
function applyPostReload() {
  const exists = new Set(statusStore.entries.map((e) => e.path))
  for (const p of [...checkedPaths.value]) {
    if (!exists.has(p)) checkedPaths.value.delete(p)
  }
  const targetPath = pendingSelectedFilePath ?? selectedFile.value?.path ?? null
  pendingSelectedFilePath = null
  if (targetPath) {
    selectedFile.value = statusStore.entries.find((e) => e.path === targetPath) ?? null
  }
}

watch(
  () => statusStore.loading,
  (now, prev) => {
    if (prev && !now) applyPostReload()
  },
)

// 文件树加载的代际令牌：切 WC 时旧结果不再覆盖新数据
const treeGen = createGeneration()

async function reloadFileTree() {
  const token = treeGen.next()
  fileTreeLoading.value = true
  try {
    const result = await api.listWorkingCopyFiles(props.workingCopy.path)
    if (!treeGen.isCurrent(token)) return
    fileTree.value = result
  } catch (e) {
    if (!treeGen.isCurrent(token)) return
    toast(e, '加载文件树失败')
  } finally {
    if (treeGen.isCurrent(token)) fileTreeLoading.value = false
  }
}

// 按 WC.id 暂存视图状态，切回时恢复——避免来回切换丢勾选/展开/选中
type WcViewState = {
  selectedFilePath: string | null
  checkedPaths: Set<string>
  expandedDirs: Set<string>
  selectedTreePath: string | null
  leftMode: 'tree' | 'changes'
}
const wcViewState = new Map<string, WcViewState>()

function snapshotCurrentWc(id: string) {
  wcViewState.set(id, {
    selectedFilePath: selectedFile.value?.path ?? null,
    checkedPaths: new Set(checkedPaths.value),
    expandedDirs: new Set(expandedDirs.value),
    selectedTreePath: selectedTreePath.value,
    leftMode: leftMode.value,
  })
}

function restoreWcState(id: string) {
  // 切换工作副本后回到纯文件列表：恢复勾选/展开等，但不自动打开 diff
  rightPane.value = 'none'
  selectedFile.value = null
  pendingSelectedFilePath = null
  const saved = wcViewState.get(id)
  if (saved) {
    checkedPaths.value = new Set(saved.checkedPaths)
    expandedDirs.value = new Set(saved.expandedDirs)
    selectedTreePath.value = saved.selectedTreePath
    leftMode.value = saved.leftMode
  } else {
    checkedPaths.value = new Set()
    expandedDirs.value = new Set()
    selectedTreePath.value = null
  }
}

watch(
  () => props.workingCopy.id,
  (newId, oldId) => {
    if (oldId) snapshotCurrentWc(oldId)
    restoreWcState(newId)
    reload().catch(toast)
  },
  { immediate: false },
)

onMounted(() => {
  reload().catch(toast)
})

// diff 加载的代际令牌：快速来回点不同文件时，旧的 diff 不应覆盖新的
const diffGen = createGeneration()

watch(selectedFile, async (entry) => {
  const token = diffGen.next()
  diffText.value = null
  baseContent.value = null
  currentContent.value = null
  if (!entry) return
  // unversioned/ignored 不能 svn diff
  if (entry.item === 'unversioned' || entry.item === 'ignored' || entry.item === 'missing') {
    diffText.value = ''
    return
  }
  diffLoading.value = true
  try {
    const diff = await api.diff(entry.path)
    if (!diffGen.isCurrent(token)) return
    diffText.value = diff
    // 尝试加载左右对比所需的两份内容
    try {
      const base = await api.baseContent(entry.path)
      if (!diffGen.isCurrent(token)) return
      baseContent.value = base
    } catch {
      if (diffGen.isCurrent(token)) baseContent.value = ''
    }
    try {
      const current = await api.readFileText(entry.path)
      if (!diffGen.isCurrent(token)) return
      currentContent.value = current
    } catch {
      if (diffGen.isCurrent(token)) currentContent.value = ''
    }
  } catch (e) {
    if (!diffGen.isCurrent(token)) return
    toast(e, '加载 diff 失败')
  } finally {
    if (diffGen.isCurrent(token)) diffLoading.value = false
  }
})

// 右侧栏列宽：diff 占近一半，提交/更新固定窄栏，关闭则文件列表独占
const gridCols = computed(() => {
  if (rightPane.value === 'diff') return 'minmax(0, 1fr) minmax(420px, 50%)'
  if (rightPane.value === 'commit' || rightPane.value === 'update') return 'minmax(0, 1fr) 360px'
  return '1fr'
})

const rightTitle = computed(() => {
  if (rightPane.value === 'diff') return selectedFile.value ? shortName(selectedFile.value.path) : '差异'
  if (rightPane.value === 'commit') return '提交'
  if (rightPane.value === 'update') return '更新'
  return ''
})

// 点击文件 → 滑出 diff
function openFileDiff(entry: SvnStatusEntry) {
  selectedFile.value = entry
  rightPane.value = 'diff'
}

function openCommit() {
  leftMode.value = 'changes'
  rightPane.value = 'commit'
}

// 提交面板选择排除未跟踪文件时，同步取消左侧勾选，保证两侧提交范围一致。
function excludeFromCommit(paths: string[]) {
  for (const path of paths) checkedPaths.value.delete(path)
}

function openUpdate() {
  rightPane.value = 'update'
}

function closeRight() {
  // 关闭 diff 时清掉选中高亮，回到纯文件列表
  if (rightPane.value === 'diff') {
    selectedFile.value = null
  }
  rightPane.value = 'none'
}

function shortName(p: string) {
  // 相对工作副本根目录的相对显示，缩短面包屑长度
  const root = props.workingCopy.path
  if (p.startsWith(root)) {
    const rel = p.slice(root.length).replace(/^[\\/]+/, '')
    return rel || p
  }
  return p
}

function fileStatus(path: string) {
  return rawFileStatus(path)
}

function fileStatusLabel(path: string) {
  const item = fileStatus(path)
  return item === 'normal' ? '' : (STATUS_LABEL[item] ?? item)
}

function fileStatusClass(path: string) {
  return STATUS_CLASSES[fileStatus(path)] ?? 'status-muted'
}

function toggleDir(entry: WorkingCopyFileEntry) {
  const next = new Set(expandedDirs.value)
  if (next.has(entry.path)) next.delete(entry.path)
  else next.add(entry.path)
  expandedDirs.value = next
}

function selectTreeEntry(entry: WorkingCopyFileEntry) {
  selectedTreePath.value = entry.path
  if (entry.kind === 'dir') {
    toggleDir(entry)
    selectedFile.value = null
    // 点目录回到纯列表，关掉可能开着的 diff
    if (rightPane.value === 'diff') {
      rightPane.value = 'none'
    }
    return
  }
  openFileDiff(
    statusByPath.value.get(entry.path) ??
      ({
        path: entry.path,
        item: fileStatus(entry.path),
        props: null,
        copied: false,
        revision: null,
        commitRevision: null,
        commitAuthor: null,
        commitDate: null,
      } satisfies SvnStatusEntry),
  )
}

function toggleTreeCheck(entry: WorkingCopyFileEntry, checked: boolean) {
  const status = statusByPath.value.get(entry.path)
  if (!status || status.item === 'normal' || status.item === 'ignored' || status.item === 'external') {
    return
  }
  toggleEntry(status, checked)
}

function createFolderParentPath() {
  if (!selectedTreePath.value) return props.workingCopy.path
  const entry = flatFileTree.value.find((row) => row.entry.path === selectedTreePath.value)?.entry
  if (entry?.kind === 'dir') return entry.path
  const idx = selectedTreePath.value.lastIndexOf('/')
  const winIdx = selectedTreePath.value.lastIndexOf('\\')
  const cut = Math.max(idx, winIdx)
  return cut > 0 ? selectedTreePath.value.slice(0, cut) : props.workingCopy.path
}

async function createFolder() {
  if (!folderName.value.trim()) return
  try {
    const created = await api.createWorkingCopyFolder(createFolderParentPath(), folderName.value)
    folderName.value = ''
    showCreateFolder.value = false
    expandedDirs.value = new Set([...expandedDirs.value, createFolderParentPath()])
    selectedTreePath.value = created
    await reload()
  } catch (e) {
    toast(e, '创建文件夹失败')
  }
}

async function revertSelected() {
  if (checkedPaths.value.size === 0) return
  const paths = [...checkedPaths.value]
  const ok = await confirm({
    title: '撤销本地修改',
    content: `这些文件将恢复到 BASE 版本，本地未提交的修改会丢失：\n${paths.join('\n')}`,
    confirmText: '确认撤销',
    destructive: true,
  })
  if (!ok) return
  try {
    await api.revert(paths)
    await reload()
  } catch (e) {
    toast(e, '撤销失败')
  }
}

async function addSelected() {
  const paths = [...checkedPaths.value].filter((path) => {
    const entry = statusStore.entries.find((e) => e.path === path)
    return entry?.item === 'unversioned'
  })
  if (paths.length === 0) return
  try {
    await api.add(paths)
    await reload()
  } catch (e) {
    toast(e, '加入版本控制失败')
  }
}

async function deleteSelected() {
  if (checkedPaths.value.size === 0) return
  const paths = [...checkedPaths.value]
  const ok = await confirm({
    title: '删除文件',
    content: `这些文件会被 svn delete 标记，未跟踪文件会从磁盘删除：\n${paths.join('\n')}`,
    confirmText: '确认删除',
    destructive: true,
  })
  if (!ok) return
  try {
    await api.delete(paths)
    await reload()
  } catch (e) {
    toast(e, '删除失败')
  }
}

async function ignoreSelected() {
  const paths = [...checkedPaths.value].filter((path) => {
    const entry = statusStore.entries.find((e) => e.path === path)
    return entry?.item === 'unversioned'
  })
  if (paths.length === 0) return
  try {
    await api.ignore(paths)
    await reload()
  } catch (e) {
    toast(e, '忽略失败')
  }
}

// ============ 右键上下文菜单 ============
const ctxOpen = ref(false)
const ctxX = ref(0)
const ctxY = ref(0)
const ctxPath = ref<string | null>(null)
const ctxItem = ref<string>('normal')

function openRowContextMenu(event: MouseEvent, path: string, item: string) {
  ctxPath.value = path
  ctxItem.value = item
  ctxX.value = event.clientX
  ctxY.value = event.clientY
  ctxOpen.value = true
}

// 是否纳入版本控制（用于决定可用的菜单项）
const VERSIONED = ['modified', 'added', 'deleted', 'replaced', 'conflicted', 'missing']

const ctxItems = computed<ContextMenuItem[]>(() => {
  const item = ctxItem.value
  const list: ContextMenuItem[] = [
    { key: 'diff', label: '查看差异', icon: Eye },
  ]
  if (item === 'unversioned') {
    list.push({ key: 'add', label: '加入版本控制', icon: Plus })
    list.push({ key: 'ignore', label: '加入忽略', icon: EyeOff })
  }
  if (VERSIONED.includes(item)) {
    list.push({ key: 'revert', label: '撤销修改', icon: Undo2, danger: true })
  }
  if (item !== 'unversioned' && item !== 'normal') {
    list.push({ key: 'delete', label: '删除', icon: Trash2, danger: true })
  }
  list.push({ key: 'sep1', separator: true })
  list.push({ key: 'copy', label: '复制路径', icon: Copy })
  list.push({ key: 'reveal', label: '在 Finder 中显示', icon: FolderSearch })
  return list
})

async function onCtxSelect(key: string) {
  const path = ctxPath.value
  if (!path) return
  switch (key) {
    case 'diff': {
      // 选中该文件并滑出 diff，交给既有的 selectedFile 监听加载内容
      openFileDiff(
        statusByPath.value.get(path) ??
          ({
            path,
            item: ctxItem.value as SvnStatusEntry['item'],
            props: null,
            copied: false,
            revision: null,
            commitRevision: null,
            commitAuthor: null,
            commitDate: null,
          } as SvnStatusEntry),
      )
      break
    }
    case 'add':
      await runSinglePathAction(() => api.add([path]), '加入版本控制失败')
      break
    case 'ignore':
      await runSinglePathAction(() => api.ignore([path]), '忽略失败')
      break
    case 'revert': {
      const ok = await confirm({
        title: '撤销本地修改',
        content: `该文件将恢复到 BASE 版本，本地未提交的修改会丢失：\n${path}`,
        confirmText: '确认撤销',
        destructive: true,
      })
      if (ok) await runSinglePathAction(() => api.revert([path]), '撤销失败')
      break
    }
    case 'delete': {
      const ok = await confirm({
        title: '删除文件',
        content: `该文件会被 svn delete 标记：\n${path}`,
        confirmText: '确认删除',
        destructive: true,
      })
      if (ok) await runSinglePathAction(() => api.delete([path]), '删除失败')
      break
    }
    case 'copy':
      try {
        await navigator.clipboard.writeText(path)
      } catch {
        // 剪贴板不可用时静默，不打断操作
      }
      break
    case 'reveal':
      try {
        await api.revealInFileManager(path)
      } catch (e) {
        toast(e, '无法在 Finder 中显示')
      }
      break
  }
}

async function runSinglePathAction(action: () => Promise<unknown>, errMsg: string) {
  try {
    await action()
    await reload()
  } catch (e) {
    toast(e, errMsg)
  }
}
</script>

<template>
  <div class="status-view" :style="{ gridTemplateColumns: gridCols }">
    <!-- 左：文件列表 -->
    <section class="file-list">
      <div class="list-toolbar">
        <div class="mode-switch">
          <Button
            size="xs"
            :variant="leftMode === 'tree' ? 'secondary' : 'ghost'"
            @click="leftMode = 'tree'"
          >
            文件
          </Button>
          <Button
            size="xs"
            :variant="leftMode === 'changes' ? 'secondary' : 'ghost'"
            @click="leftMode = 'changes'"
          >
            变更
          </Button>
        </div>
        <Button
          v-if="leftMode === 'tree'"
          size="xs"
          variant="ghost"
          class="toolbar-action"
          @click="showCreateFolder = true"
        >
          <FolderPlus class="icon-xs" />
          新建文件夹
        </Button>
        <span class="spacer" />

        <!-- 勾选文件后才出现的批量操作 -->
        <div v-if="checkedPaths.size > 0" class="check-actions">
          <Button
            size="xs"
            variant="ghost"
            class="success-action"
            :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
            @click="addSelected"
          >
            Add
          </Button>
          <Button
            size="xs"
            variant="ghost"
            :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
            @click="ignoreSelected"
          >
            忽略
          </Button>
          <Button size="xs" variant="ghost" class="danger-action" @click="deleteSelected">
            删除
          </Button>
          <Button size="xs" variant="ghost" class="warning-action" @click="revertSelected">
            撤销
          </Button>
          <span class="tool-sep" />
        </div>

        <Button
          size="xs"
          :variant="rightPane === 'commit' ? 'secondary' : 'ghost'"
          class="toolbar-action"
          @click="openCommit"
        >
          提交
        </Button>
        <Button
          size="xs"
          :variant="rightPane === 'update' ? 'secondary' : 'ghost'"
          class="toolbar-action"
          @click="openUpdate"
        >
          更新
        </Button>
        <span class="tool-sep" />
        <Switch :model-value="statusStore.showUnversioned" @update:model-value="onToggleUnversioned" />
        <span class="hint">未跟踪</span>
        <Button
          size="xs"
          :variant="statusStore.showIgnored ? 'secondary' : 'ghost'"
          class="toolbar-action"
          @click="onToggleIgnored"
        >
          <EyeOff class="icon-xs" />
          已忽略
        </Button>
        <Button size="xs" variant="ghost" class="toolbar-action" @click="reload">
          <RefreshCw class="icon-xs" />
          刷新
        </Button>
      </div>
      <div class="file-table-head">
        <div
          v-for="column in FILE_COLUMNS"
          :key="column.key"
          :class="['head-cell', column.className, { active: sortState.key === column.key, 'head-name': column.key === 'name' }]"
        >
          <Checkbox
            v-if="column.key === 'name' && leftMode === 'changes'"
            :model-value="allChecked ? true : (visibleCheckedCount > 0 ? 'indeterminate' : false)"
            title="全选"
            @update:model-value="(v) => toggleAll(v === true)"
          />
          <span
            v-else-if="column.key === 'name'"
            class="head-check-placeholder"
          />
          <button
            type="button"
            class="head-sort"
            @click="toggleSort(column.key)"
          >
            <span>{{ column.label }}</span>
            <component :is="sortIcon(column.key)" class="sort-icon" />
          </button>
        </div>
      </div>
      <div v-if="leftMode === 'tree'" ref="treeScrollRef" class="tree-scroll virtual-scroll">
        <LoadingSpinner v-if="fileTreeLoading" />
        <EmptyState
          v-else-if="flatFileTree.length === 0"
          description="工作副本目录为空"
        />
        <div
          v-else
          class="virtual-stage"
          :style="{ height: `${treeVirtualizer.getTotalSize()}px` }"
        >
          <div
            v-for="vRow in treeVirtualizer.getVirtualItems()"
            :key="flatFileTree[vRow.index].entry.path"
            class="virtual-slot"
            :style="{ transform: `translateY(${vRow.start}px)`, height: `${vRow.size}px` }"
          >
            <div
              :class="['tree-row', 'tree-row-virt', { active: selectedTreePath === flatFileTree[vRow.index].entry.path }]"
              @click="selectTreeEntry(flatFileTree[vRow.index].entry)"
              @contextmenu.prevent="flatFileTree[vRow.index].entry.kind === 'file'
                && openRowContextMenu($event, flatFileTree[vRow.index].entry.path, fileStatus(flatFileTree[vRow.index].entry.path))"
            >
              <div class="name-cell" :style="{ paddingLeft: `${8 + flatFileTree[vRow.index].depth * 16}px` }">
                <ChevronRight
                  v-if="flatFileTree[vRow.index].entry.kind === 'dir'"
                  :class="['tree-caret', { expanded: expandedDirs.has(flatFileTree[vRow.index].entry.path) }]"
                />
                <span v-else class="tree-caret placeholder" />
                <component
                  :is="flatFileTree[vRow.index].entry.kind === 'dir'
                    ? (expandedDirs.has(flatFileTree[vRow.index].entry.path) ? FolderOpen : Folder)
                    : FileText"
                  :class="['tree-icon', flatFileTree[vRow.index].entry.kind === 'dir' ? 'tree-icon-dir' : 'tree-icon-file']"
                />
                <Checkbox
                  :disabled="!statusByPath.has(flatFileTree[vRow.index].entry.path) || ['normal', 'ignored', 'external'].includes(fileStatus(flatFileTree[vRow.index].entry.path))"
                  :model-value="checkedPaths.has(flatFileTree[vRow.index].entry.path)"
                  @update:model-value="(v) => toggleTreeCheck(flatFileTree[vRow.index].entry, v === true)"
                  @click.stop
                />
                <span class="file-path mono" :title="flatFileTree[vRow.index].entry.path">
                  {{ flatFileTree[vRow.index].entry.name }}
                </span>
              </div>
              <span class="file-meta col-date">{{ formatFileDate(fileDate(flatFileTree[vRow.index].entry)) }}</span>
              <span class="file-meta col-size mono">{{ formatFileSize(flatFileTree[vRow.index].entry.size, flatFileTree[vRow.index].entry.kind) }}</span>
              <span class="file-meta col-kind">{{ fileKindLabel(flatFileTree[vRow.index].entry) }}</span>
              <span class="file-meta col-rev mono">{{ fileRevision(flatFileTree[vRow.index].entry) ?? '--' }}</span>
              <span class="file-meta col-status">
                <Badge
                  v-if="fileStatusLabel(flatFileTree[vRow.index].entry.path)"
                  variant="outline"
                  :class="fileStatusClass(flatFileTree[vRow.index].entry.path)"
                >
                  {{ fileStatusLabel(flatFileTree[vRow.index].entry.path) }}
                </Badge>
                <span v-else class="empty-meta">--</span>
              </span>
              <span class="file-meta col-author">{{ fileAuthor(flatFileTree[vRow.index].entry) || '--' }}</span>
            </div>
          </div>
        </div>
      </div>
      <div v-else ref="changesScrollRef" class="list-scroll virtual-scroll">
        <LoadingSpinner v-if="statusStore.loading" />
        <EmptyState
          v-else-if="flatChanges.length === 0"
          description="工作区干净，没有变更"
        />
        <div
          v-else
          class="virtual-stage"
          :style="{ height: `${changesVirtualizer.getTotalSize()}px` }"
        >
          <template v-for="vRow in changesVirtualizer.getVirtualItems()" :key="vRow.index">
            <div
              class="virtual-slot"
              :style="{ transform: `translateY(${vRow.start}px)`, height: `${vRow.size}px` }"
            >
              <div
                :class="['file-row', 'file-row-virt', { active: selectedFile?.path === (flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.path }]"
                @click="openFileDiff((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry)"
                @contextmenu.prevent="openRowContextMenu(
                  $event,
                  (flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.path,
                  (flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.item,
                )"
              >
                <div class="name-cell change-name">
                  <Checkbox
                    :disabled="['normal', 'ignored', 'external'].includes((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.item)"
                    :model-value="checkedPaths.has((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.path)"
                    @update:model-value="(v) => toggleEntry((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry, v === true)"
                    @click.stop
                  />
                  <span class="file-path mono" :title="(flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.path">
                    {{ shortName((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.path) }}
                  </span>
                </div>
                <span class="file-meta col-date">
                  {{ formatFileDate(changeModifiedAt((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry)) }}
                </span>
                <span class="file-meta col-size mono">
                  {{ formatFileSize(changeSize((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry), changeFileEntry((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry)?.kind) }}
                </span>
                <span class="file-meta col-kind">
                  {{ changeKindLabel((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry) }}
                </span>
                <span class="file-meta col-rev mono">
                  {{ changeRevision((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry) ?? '--' }}
                </span>
                <span class="file-meta col-status">
                  <Badge
                    variant="outline"
                    :class="STATUS_CLASSES[(flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.item] ?? 'status-muted'"
                  >
                    {{ STATUS_LABEL[(flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.item] ?? (flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry.item }}
                  </Badge>
                </span>
                <span class="file-meta col-author">
                  {{ changeAuthor((flatChanges[vRow.index] as Extract<ChangeRow, { kind: 'entry' }>).entry) || '--' }}
                </span>
              </div>
            </div>
          </template>
        </div>
      </div>
    </section>

    <!-- 右侧按需栏：diff / 提交 / 更新，可关闭 -->
    <aside v-if="rightPane !== 'none'" class="right-pane">
      <div class="right-head">
        <span class="right-title" :title="selectedFile?.path">{{ rightTitle }}</span>
        <button class="right-close" type="button" title="关闭" @click="closeRight">
          <X class="icon-xs" />
        </button>
      </div>
      <div class="right-body">
        <DiffViewer
          v-if="rightPane === 'diff'"
          :diff-text="diffText"
          :base-content="baseContent"
          :current-content="currentContent"
          :filename="selectedFile?.path"
          :loading="diffLoading"
        />
        <CommitPanel
          v-else-if="rightPane === 'commit'"
          :working-copy="workingCopy"
          :checked-paths="checkedCommittablePaths"
          :unversioned-paths="checkedUnversionedPaths"
          @exclude="excludeFromCommit"
          @done="reloadAfterWorkingCopyChange"
        />
        <UpdatePanel
          v-else-if="rightPane === 'update'"
          :working-copy="workingCopy"
          :checked-paths="[...checkedPaths]"
          @done="reloadAfterWorkingCopyChange"
        />
      </div>
    </aside>

    <Dialog v-model:open="showCreateFolder">
      <DialogContent class="folder-modal">
        <DialogHeader>
          <DialogTitle>新建文件夹</DialogTitle>
        </DialogHeader>
        <div class="folder-form">
          <div class="folder-parent mono" :title="createFolderParentPath()">
            {{ createFolderParentPath() }}
          </div>
          <Input
            v-model="folderName"
            placeholder="文件夹名称"
            @keyup.enter="createFolder"
          />
        </div>
        <DialogFooter>
          <div class="folder-actions">
          <Button variant="outline" @click="showCreateFolder = false">取消</Button>
          <Button :disabled="!folderName.trim()" @click="createFolder">
            创建
          </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <ContextMenu
      v-model:open="ctxOpen"
      :x="ctxX"
      :y="ctxY"
      :items="ctxItems"
      @select="onCtxSelect"
    />
  </div>
</template>

<style scoped>
/* ============ 三栏容器 ============ */
.status-view {
  display: grid;
  /* 列宽由 :style 绑定的 gridCols 控制：纯列表 1fr，或列表 + 右侧按需栏 */
  grid-template-columns: 1fr;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
}
.file-list,
.right-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.file-list {
  --file-table-columns:
    minmax(220px, 1.65fr)
    minmax(128px, 0.82fr)
    minmax(72px, 0.42fr)
    minmax(86px, 0.48fr)
    minmax(82px, 0.42fr)
    minmax(82px, 0.44fr)
    minmax(96px, 0.52fr);
}
.right-pane {
  border-left: var(--hairline) solid var(--stroke-soft);
  overflow: hidden;
}

/* ============ 右侧按需栏头部 ============ */
.right-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex: none;
  min-height: 36px;
  padding: 0 6px 0 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
}
.right-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-callout);
  font-weight: 600;
  color: var(--fg-strong);
}
.right-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex: none;
  border: 0;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--fg-muted);
  cursor: pointer;
  transition: background-color 120ms ease-out, color 120ms ease-out;
}
.right-close:hover {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg);
}
.right-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.right-body > * {
  flex: 1;
  min-height: 0;
}

/* ============ 顶部工具条 ============ */
.list-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  flex: none;
  padding: 0 10px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: transparent;
  font-size: var(--fs-callout);
  overflow: hidden;
}
.spacer {
  flex: 1;
}
.check-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}
.tool-sep {
  width: 1px;
  height: 16px;
  background: var(--stroke-soft);
  margin: 0 3px;
  flex: none;
}
.hint {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  user-select: none;
}

/* ============ 文件表头 ============ */
.file-table-head {
  display: grid;
  grid-template-columns: var(--file-table-columns);
  gap: 8px;
  align-items: center;
  height: 30px;
  flex: none;
  padding: 0 14px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: color-mix(in srgb, var(--mat-toolbar) 82%, transparent);
  user-select: none;
}
.head-cell {
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  font-weight: 600;
  text-align: left;
}
.head-cell:hover,
.head-cell.active {
  color: var(--fg-strong);
}
.head-name {
  gap: 7px;
  padding-left: 8px;
}
.head-check-placeholder {
  width: 14px;
  height: 14px;
  flex: none;
}
.head-sort {
  min-width: 0;
  height: 100%;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.head-sort span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sort-icon {
  width: 11px;
  height: 11px;
  flex: none;
  opacity: 0.62;
}
.head-cell.active .sort-icon {
  opacity: 1;
}

/* ============ mode-switch 胶囊 segmented control ============ */
.mode-switch {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.06);
  border: var(--hairline) solid var(--stroke-soft);
}
.dark .mode-switch {
  background: rgba(255, 255, 255, 0.05);
}
.mode-switch :deep(button) {
  height: 22px !important;
  padding: 0 10px !important;
  border-radius: 5px !important;
  font-size: var(--fs-caption);
  font-weight: 500;
  background: transparent;
  color: var(--fg-muted);
  border: 0;
  box-shadow: none;
  transition: background-color 140ms ease-out, color 140ms ease-out, box-shadow 160ms ease-out;
}
.mode-switch :deep(button:hover) {
  color: var(--fg);
  background: transparent;
}
.mode-switch :deep(button.ctl-secondary),
.mode-switch :deep(button[data-active='true']) {
  color: var(--fg-strong);
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 1.5px rgba(0, 0, 0, 0.06);
}
.dark .mode-switch :deep(button.ctl-secondary) {
  box-shadow:
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.08),
    0 1px 1.5px rgba(0, 0, 0, 0.4);
}

.inline-check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--fg);
  font-size: var(--fs-caption);
  user-select: none;
}

/* ============ 列表滚动区 ============ */
.list-scroll,
.tree-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 0;
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
  padding: 0 6px;
  display: flex;
  align-items: stretch;
}
.virtual-slot > * {
  flex: 1;
  min-width: 0;
}
/* 虚拟行覆盖原 margin（margin 在绝对定位里会导致计算偏差） */
.tree-row-virt,
.file-row-virt {
  margin: 0 !important;
}

/* ============ 分组（变更视图）============ */
.group {
  margin-bottom: 6px;
}
.group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px 4px;
  background: transparent;
  border-bottom: 0;
}
.group-count {
  font-size: var(--fs-caption);
  font-feature-settings: 'tnum';
  color: var(--fg-subtle);
}

/* ============ 文件行（变更视图 & 树视图）============ */
.file-row,
.tree-row {
  display: grid;
  grid-template-columns: var(--file-table-columns);
  gap: 8px;
  align-items: center;
  min-height: 28px;
  margin: 1px 6px;
  padding: 3px 8px;
  border-radius: var(--radius-row);
  border: 0;
  cursor: pointer;
  background: transparent;
  font-size: var(--fs-callout);
  transition: background-color 120ms ease-out;
}
.file-row:hover,
.tree-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.file-row.active,
.tree-row.active {
  background: var(--accent);
}
.name-cell {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}
.change-name {
  padding-left: 8px;
}
.file-meta {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--fg-muted);
  font-size: var(--fs-callout);
}
.col-size,
.col-rev {
  justify-content: flex-end;
  justify-self: end;
  text-align: right;
  font-feature-settings: 'tnum';
}
.col-status {
  display: inline-flex;
  align-items: center;
}
.empty-meta {
  color: var(--fg-subtle);
}

/* 选中态：文字、icon、checkbox 反白 */
.file-row.active .file-path,
.tree-row.active .file-path,
.file-row.active .file-meta,
.tree-row.active .file-meta,
.file-row.active .empty-meta,
.tree-row.active .empty-meta {
  color: #fff;
}
.tree-row.active .tree-icon-dir,
.tree-row.active .tree-icon-file,
.tree-row.active .tree-caret {
  color: rgba(255, 255, 255, 0.92);
}
.file-row.active :deep([data-state]),
.tree-row.active :deep([data-state]) {
  /* checkbox 在 active 行的边框/勾色微调 */
  border-color: rgba(255, 255, 255, 0.7);
}

/* ============ 文件名 / 树元素 ============ */
.tree-caret {
  width: 12px;
  height: 12px;
  color: var(--fg-muted);
  transition: transform 140ms ease-out;
}
.tree-caret.expanded {
  transform: rotate(90deg);
}
.tree-caret.placeholder {
  background: transparent;
}
.tree-icon {
  width: 14px;
  height: 14px;
  flex: none;
}
.tree-icon-dir {
  color: var(--folder);
}
.tree-icon-file {
  color: var(--fg-muted);
}
.file-path {
  flex: 1;
  font-size: var(--fs-callout);
  font-family: var(--mono);
  color: var(--fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ============ 工具栏小按钮 ============ */
.toolbar-action {
  gap: 4px;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
}
.toolbar-action:hover {
  color: var(--fg-strong);
}
.icon-xs {
  width: 12px;
  height: 12px;
}

/* ============ Status Pill（行内 / 组标题统一）============
   tinted pill：浅底 + 同色字 + 0.5px 同色描边 + 圆胶囊
*/
.status-modified,
.status-added,
.status-deleted,
.status-warning,
.status-muted {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 7px;
  font-size: var(--fs-caption);
  font-weight: 500;
  line-height: 1;
  border-radius: var(--radius-pill);
  border: var(--hairline) solid transparent;
  background: var(--mat-elevated);
}
.status-modified {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 28%, transparent);
}
.status-added {
  color: var(--success);
  background: var(--success-soft);
  border-color: color-mix(in srgb, var(--success) 32%, transparent);
}
.status-deleted {
  color: var(--danger);
  background: var(--danger-soft);
  border-color: color-mix(in srgb, var(--danger) 32%, transparent);
}
.status-warning {
  color: var(--warning);
  background: var(--warning-soft);
  border-color: color-mix(in srgb, var(--warning) 34%, transparent);
}
.status-muted {
  color: var(--fg-muted);
  background: color-mix(in srgb, var(--fg) 6%, transparent);
  border-color: color-mix(in srgb, var(--fg) 12%, transparent);
}

/* 选中行内的 status pill 切换为白底变体 */
.file-row.active .status-modified,
.file-row.active .status-added,
.file-row.active .status-deleted,
.file-row.active .status-warning,
.file-row.active .status-muted,
.tree-row.active .status-modified,
.tree-row.active .status-added,
.tree-row.active .status-deleted,
.tree-row.active .status-warning,
.tree-row.active .status-muted {
  background: rgba(255, 255, 255, 0.22);
  color: #fff;
  border-color: rgba(255, 255, 255, 0.32);
}

/* ============ 工具栏中的语义动作色（提交/更新/Add 等）============ */
.success-action {
  color: var(--success);
}
.danger-action {
  color: var(--danger);
}
.warning-action {
  color: var(--warning);
}

/* ============ 新建文件夹 Dialog ============ */
.folder-modal {
  width: min(520px, calc(100vw - 32px));
  border-radius: var(--radius-window);
  background: var(--mat-popover);
  box-shadow: var(--shadow-modal);
  backdrop-filter: var(--vibrancy-popover);
  -webkit-backdrop-filter: var(--vibrancy-popover);
}
.folder-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.folder-parent {
  padding: 8px 10px;
  border-radius: var(--radius-control);
  background: color-mix(in srgb, var(--fg) 5%, transparent);
  border: var(--hairline) solid var(--stroke-soft);
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.folder-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
