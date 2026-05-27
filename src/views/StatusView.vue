<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  ChevronRight,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  RefreshCw,
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
import { confirm } from '@/composables/use-confirm-dialog'
import DiffViewer from '../components/DiffViewer.vue'
import CommitPanel from '../components/CommitPanel.vue'
import UpdatePanel from '../components/UpdatePanel.vue'

import { api } from '../api/svn'
import { useStatusStore } from '../stores/status'
import { useErrorToast } from '../composables/use-error-toast'
import type { SvnStatusEntry, WorkingCopyEntry, WorkingCopyFileEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry }>()

const statusStore = useStatusStore()
const toast = useErrorToast()

const selectedFile = ref<SvnStatusEntry | null>(null)
const checkedPaths = ref<Set<string>>(new Set())
const diffText = ref<string | null>(null)
const baseContent = ref<string | null>(null)
const currentContent = ref<string | null>(null)
const diffLoading = ref(false)
const showUpdate = ref(false)
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
  ignored: '忽略',
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

const grouped = computed(() => {
  const map = new Map<string, SvnStatusEntry[]>()
  for (const e of statusStore.entries) {
    const k = e.item || 'normal'
    if (!map.has(k)) map.set(k, [])
    map.get(k)!.push(e)
  }
  return STATUS_ORDER.filter((s) => map.has(s)).map((s) => ({
    item: s,
    label: STATUS_LABEL[s] ?? s,
    className: STATUS_CLASSES[s] ?? 'status-muted',
    entries: map.get(s)!.sort((a, b) => a.path.localeCompare(b.path)),
  }))
})

// 可提交的项（不能 commit unversioned/ignored/normal/missing/external）
const COMMITTABLE = new Set([
  'modified',
  'added',
  'deleted',
  'replaced',
  'conflicted',
])

const allCommittable = computed(() =>
  statusStore.entries.filter((e) => COMMITTABLE.has(e.item)),
)
const allChecked = computed(
  () =>
    allCommittable.value.length > 0 &&
    allCommittable.value.every((e) => checkedPaths.value.has(e.path)),
)
const checkedCommittablePaths = computed(() =>
  [...checkedPaths.value].filter((path) => {
    const entry = statusStore.entries.find((e) => e.path === path)
    return entry ? COMMITTABLE.has(entry.item) : false
  }),
)
const statusByPath = computed(() => {
  const map = new Map<string, SvnStatusEntry>()
  for (const entry of statusStore.entries) map.set(entry.path, entry)
  return map
})
const flatFileTree = computed(() => {
  const rows: { entry: WorkingCopyFileEntry; depth: number }[] = []
  function visit(entries: WorkingCopyFileEntry[], depth: number) {
    for (const entry of entries) {
      rows.push({ entry, depth })
      if (entry.kind === 'dir' && expandedDirs.value.has(entry.path)) {
        visit(entry.children, depth + 1)
      }
    }
  }
  visit(fileTree.value, 0)
  return rows
})

function toggleAll(v: boolean) {
  if (v) {
    for (const e of allCommittable.value) checkedPaths.value.add(e.path)
  } else {
    for (const e of allCommittable.value) checkedPaths.value.delete(e.path)
  }
}

function toggleEntry(e: SvnStatusEntry, v: boolean) {
  if (v) checkedPaths.value.add(e.path)
  else checkedPaths.value.delete(e.path)
}

async function reload() {
  await statusStore.reload(props.workingCopy.path)
  await reloadFileTree()
  // 清掉已经不存在的勾选
  const exists = new Set(statusStore.entries.map((e) => e.path))
  for (const p of [...checkedPaths.value]) {
    if (!exists.has(p)) checkedPaths.value.delete(p)
  }
  // 选中状态保留：找回当前选中文件
  if (selectedFile.value) {
    const found = statusStore.entries.find((e) => e.path === selectedFile.value!.path)
    selectedFile.value = found ?? null
  }
}

async function reloadFileTree() {
  fileTreeLoading.value = true
  try {
    fileTree.value = await api.listWorkingCopyFiles(props.workingCopy.path)
  } catch (e) {
    toast(e, '加载文件树失败')
  } finally {
    fileTreeLoading.value = false
  }
}

watch(
  () => props.workingCopy.id,
  () => {
    selectedFile.value = null
    checkedPaths.value = new Set()
    reload().catch(toast)
  },
  { immediate: false },
)

onMounted(() => {
  reload().catch(toast)
})

watch(selectedFile, async (entry) => {
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
    diffText.value = await api.diff(entry.path)
    // 尝试加载左右对比所需的两份内容
    try {
      baseContent.value = await api.baseContent(entry.path)
    } catch {
      baseContent.value = ''
    }
    try {
      currentContent.value = await api.readFileText(entry.path)
    } catch {
      currentContent.value = ''
    }
  } catch (e) {
    toast(e, '加载 diff 失败')
  } finally {
    diffLoading.value = false
  }
})

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
  return statusByPath.value.get(path)?.item ?? 'normal'
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
    return
  }
  selectedFile.value =
    statusByPath.value.get(entry.path) ??
    ({
      path: entry.path,
      item: 'normal',
      props: null,
      copied: false,
      revision: null,
      commitRevision: null,
      commitAuthor: null,
      commitDate: null,
    } satisfies SvnStatusEntry)
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
</script>

<template>
  <div class="status-view">
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
        <span class="spacer" />
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
        <label
          v-if="leftMode === 'changes'"
          class="inline-check"
        >
          <Checkbox
            :model-value="allChecked ? true : ([...checkedPaths].length > 0 ? 'indeterminate' : false)"
            @update:model-value="(v) => toggleAll(v === true)"
          />
          全选
        </label>
        <Switch v-model="statusStore.showUnversioned" @update:model-value="reload" />
        <span class="hint">显示未跟踪</span>
        <Button size="xs" variant="ghost" class="toolbar-action" @click="reload">
          <RefreshCw class="icon-xs" />
          刷新
        </Button>
      </div>
      <div v-if="leftMode === 'tree'" class="tree-scroll">
        <LoadingSpinner v-if="fileTreeLoading" />
        <EmptyState
          v-else-if="flatFileTree.length === 0"
          description="工作副本目录为空"
        />
        <div
          v-for="row in flatFileTree"
          v-else
          :key="row.entry.path"
          :class="['tree-row', { active: selectedTreePath === row.entry.path }]"
          :style="{ paddingLeft: `${10 + row.depth * 16}px` }"
          @click="selectTreeEntry(row.entry)"
        >
          <ChevronRight
            v-if="row.entry.kind === 'dir'"
            :class="['tree-caret', { expanded: expandedDirs.has(row.entry.path) }]"
          />
          <span v-else class="tree-caret placeholder" />
          <component
            :is="row.entry.kind === 'dir'
              ? (expandedDirs.has(row.entry.path) ? FolderOpen : Folder)
              : FileText"
            :class="['tree-icon', row.entry.kind === 'dir' ? 'tree-icon-dir' : 'tree-icon-file']"
          />
          <Checkbox
            :disabled="!statusByPath.has(row.entry.path) || ['normal', 'ignored', 'external'].includes(fileStatus(row.entry.path))"
            :model-value="checkedPaths.has(row.entry.path)"
            @update:model-value="(v) => toggleTreeCheck(row.entry, v === true)"
            @click.stop
          />
          <span class="file-path mono" :title="row.entry.path">{{ row.entry.name }}</span>
          <Badge
            v-if="fileStatusLabel(row.entry.path)"
            variant="outline"
            :class="fileStatusClass(row.entry.path)"
          >
            {{ fileStatusLabel(row.entry.path) }}
          </Badge>
        </div>
      </div>
      <div v-else class="list-scroll">
        <LoadingSpinner v-if="statusStore.loading" />
        <EmptyState
          v-else-if="grouped.length === 0"
          description="工作区干净，没有变更"
        />
        <div v-for="group in grouped" :key="group.item" class="group">
          <div class="group-header">
            <Badge variant="outline" :class="group.className">{{ group.label }}</Badge>
            <span class="group-count mono">{{ group.entries.length }}</span>
          </div>
          <div
            v-for="e in group.entries"
            :key="e.path"
            :class="['file-row', { active: selectedFile?.path === e.path }]"
            @click="selectedFile = e"
          >
            <Checkbox
              :disabled="e.item === 'normal' || e.item === 'ignored' || e.item === 'external'"
              :model-value="checkedPaths.has(e.path)"
              @update:model-value="(v) => toggleEntry(e, v === true)"
              @click.stop
            />
            <span class="file-path mono" :title="e.path">{{ shortName(e.path) }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- 中：diff -->
    <section class="diff-pane">
      <DiffViewer
        :diff-text="diffText"
        :base-content="baseContent"
        :current-content="currentContent"
        :filename="selectedFile?.path"
        :loading="diffLoading"
      />
    </section>

    <!-- 右：commit/update 面板 -->
    <section class="side-pane">
      <div class="side-tabs">
        <Button
          size="sm"
          :variant="!showUpdate ? 'secondary' : 'ghost'"
          @click="showUpdate = false"
        >
          提交
        </Button>
        <Button
          size="sm"
          :variant="showUpdate ? 'secondary' : 'ghost'"
          @click="showUpdate = true"
        >
          更新
        </Button>
        <span class="spacer" />
        <Button
          size="sm"
          variant="ghost"
          class="success-action"
          :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
          @click="addSelected"
        >
          Add
        </Button>
        <Button
          size="sm"
          variant="ghost"
          :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
          @click="ignoreSelected"
        >
          忽略
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="danger-action"
          :disabled="checkedPaths.size === 0"
          @click="deleteSelected"
        >
          删除
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="warning-action"
          :disabled="checkedPaths.size === 0"
          @click="revertSelected"
        >
          撤销
        </Button>
      </div>
      <CommitPanel
        v-if="!showUpdate"
        :working-copy="workingCopy"
        :checked-paths="checkedCommittablePaths"
        @done="reload"
      />
      <UpdatePanel
        v-else
        :working-copy="workingCopy"
        :checked-paths="[...checkedPaths]"
        @done="reload"
      />
    </section>

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
  </div>
</template>

<style scoped>
/* ============ 三栏容器 ============ */
.status-view {
  display: grid;
  grid-template-columns: 320px 1fr 340px;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
}
.file-list,
.diff-pane,
.side-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.file-list {
  border-right: var(--hairline) solid var(--stroke-soft);
}
.side-pane {
  border-left: var(--hairline) solid var(--stroke-soft);
}

/* ============ 顶部工具条（list-toolbar / side-tabs）============ */
.list-toolbar,
.side-tabs {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  flex: none;
  padding: 0 10px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: transparent;
  font-size: var(--fs-callout);
}
.spacer {
  flex: 1;
}
.hint {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  user-select: none;
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
.mode-switch :deep(button.bg-secondary),
.mode-switch :deep(button[data-active='true']) {
  color: var(--fg-strong);
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 1.5px rgba(0, 0, 0, 0.06);
}
.dark .mode-switch :deep(button.bg-secondary) {
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
.file-row {
  display: flex;
  gap: 8px;
  align-items: center;
  min-height: 28px;
  margin: 1px 6px;
  padding: 3px 8px;
  border-radius: var(--radius-row);
  border: 0;
  cursor: pointer;
  background: transparent;
  transition: background-color 120ms ease-out;
}
.file-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.file-row.active {
  background: var(--accent);
}

.tree-row {
  display: grid;
  grid-template-columns: 12px 16px 22px minmax(0, 1fr) auto;
  gap: 7px;
  align-items: center;
  min-height: 26px;
  margin: 1px 6px;
  padding: 2px 8px;
  border-radius: var(--radius-row);
  border: 0;
  cursor: pointer;
  background: transparent;
  font-size: var(--fs-callout);
  transition: background-color 120ms ease-out;
}
.tree-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.tree-row.active {
  background: var(--accent);
}

/* 选中态：文字、icon、checkbox 反白 */
.file-row.active .file-path,
.tree-row.active .file-path {
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
