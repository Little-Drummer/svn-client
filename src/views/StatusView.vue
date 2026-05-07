<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  NButton,
  NCheckbox,
  NEmpty,
  NInput,
  NModal,
  NScrollbar,
  NSpin,
  NSwitch,
  NTag,
  useDialog,
} from 'naive-ui'

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
const dialog = useDialog()

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
const STATUS_COLORS: Record<string, 'default' | 'success' | 'info' | 'warning' | 'error'> = {
  modified: 'info',
  added: 'success',
  deleted: 'error',
  replaced: 'warning',
  conflicted: 'error',
  missing: 'warning',
  obstructed: 'warning',
  unversioned: 'default',
  ignored: 'default',
  external: 'default',
  incomplete: 'warning',
  normal: 'default',
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
    color: STATUS_COLORS[s] ?? 'default',
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

function fileStatusColor(path: string): 'default' | 'success' | 'info' | 'warning' | 'error' {
  return STATUS_COLORS[fileStatus(path)] ?? 'default'
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
  dialog.warning({
    title: '撤销本地修改',
    content: `这些文件将恢复到 BASE 版本，本地未提交的修改会丢失：\n${paths.join('\n')}`,
    positiveText: '确认撤销',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await api.revert(paths)
        await reload()
      } catch (e) {
        toast(e, '撤销失败')
      }
    },
  })
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
  dialog.warning({
    title: '删除文件',
    content: `这些文件会被 svn delete 标记，未跟踪文件会从磁盘删除：\n${paths.join('\n')}`,
    positiveText: '确认删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await api.delete(paths)
        await reload()
      } catch (e) {
        toast(e, '删除失败')
      }
    },
  })
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
          <n-button
            size="tiny"
            :type="leftMode === 'tree' ? 'primary' : 'tertiary'"
            @click="leftMode = 'tree'"
          >
            文件
          </n-button>
          <n-button
            size="tiny"
            :type="leftMode === 'changes' ? 'primary' : 'tertiary'"
            @click="leftMode = 'changes'"
          >
            变更
          </n-button>
        </div>
        <span class="spacer" />
        <n-button
          v-if="leftMode === 'tree'"
          size="tiny"
          tertiary
          type="primary"
          @click="showCreateFolder = true"
        >
          新建文件夹
        </n-button>
        <n-checkbox
          v-if="leftMode === 'changes'"
          :checked="allChecked"
          :indeterminate="!allChecked && [...checkedPaths].length > 0"
          @update:checked="(v: boolean) => toggleAll(v)"
        >
          全选
        </n-checkbox>
        <n-switch
          v-model:value="statusStore.showUnversioned"
          size="small"
          @update:value="reload"
        />
        <span class="hint">显示未跟踪</span>
        <n-button size="tiny" tertiary @click="reload">刷新</n-button>
      </div>
      <div v-if="leftMode === 'tree'" class="tree-scroll">
        <n-spin v-if="fileTreeLoading" />
        <n-empty
          v-else-if="flatFileTree.length === 0"
          description="工作副本目录为空"
          size="small"
        />
        <div
          v-for="row in flatFileTree"
          v-else
          :key="row.entry.path"
          :class="['tree-row', { active: selectedTreePath === row.entry.path }]"
          :style="{ paddingLeft: `${10 + row.depth * 16}px` }"
          @click="selectTreeEntry(row.entry)"
        >
          <span
            v-if="row.entry.kind === 'dir'"
            :class="['tree-caret', { collapsed: !expandedDirs.has(row.entry.path) }]"
          />
          <span v-else class="tree-caret placeholder" />
          <span :class="['tree-icon', row.entry.kind === 'dir' ? 'dir-icon' : 'file-icon']" />
          <n-checkbox
            :disabled="!statusByPath.has(row.entry.path) || ['normal', 'ignored', 'external'].includes(fileStatus(row.entry.path))"
            :checked="checkedPaths.has(row.entry.path)"
            @update:checked="(v: boolean) => toggleTreeCheck(row.entry, v)"
            @click.stop
          />
          <span class="file-path mono" :title="row.entry.path">{{ row.entry.name }}</span>
          <n-tag
            v-if="fileStatusLabel(row.entry.path)"
            size="tiny"
            :type="fileStatusColor(row.entry.path)"
          >
            {{ fileStatusLabel(row.entry.path) }}
          </n-tag>
        </div>
      </div>
      <n-scrollbar v-else class="list-scroll">
        <n-spin v-if="statusStore.loading" />
        <n-empty
          v-else-if="grouped.length === 0"
          description="工作区干净，没有变更"
          size="small"
        />
        <div v-for="group in grouped" :key="group.item" class="group">
          <div class="group-header">
            <n-tag size="small" :type="group.color">{{ group.label }}</n-tag>
            <span class="group-count mono">{{ group.entries.length }}</span>
          </div>
          <div
            v-for="e in group.entries"
            :key="e.path"
            :class="['file-row', { active: selectedFile?.path === e.path }]"
            @click="selectedFile = e"
          >
            <n-checkbox
              :disabled="e.item === 'normal' || e.item === 'ignored' || e.item === 'external'"
              :checked="checkedPaths.has(e.path)"
              @update:checked="(v: boolean) => toggleEntry(e, v)"
              @click.stop
            />
            <span class="file-path mono" :title="e.path">{{ shortName(e.path) }}</span>
          </div>
        </div>
      </n-scrollbar>
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
        <n-button
          size="small"
          :type="!showUpdate ? 'primary' : 'tertiary'"
          @click="showUpdate = false"
        >
          提交
        </n-button>
        <n-button
          size="small"
          :type="showUpdate ? 'primary' : 'tertiary'"
          @click="showUpdate = true"
        >
          更新
        </n-button>
        <span class="spacer" />
        <n-button
          size="small"
          tertiary
          type="success"
          :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
          @click="addSelected"
        >
          Add
        </n-button>
        <n-button
          size="small"
          tertiary
          :disabled="[...checkedPaths].every((path) => statusStore.entries.find((e) => e.path === path)?.item !== 'unversioned')"
          @click="ignoreSelected"
        >
          忽略
        </n-button>
        <n-button
          size="small"
          tertiary
          type="error"
          :disabled="checkedPaths.size === 0"
          @click="deleteSelected"
        >
          删除
        </n-button>
        <n-button
          size="small"
          tertiary
          type="warning"
          :disabled="checkedPaths.size === 0"
          @click="revertSelected"
        >
          撤销
        </n-button>
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

    <n-modal v-model:show="showCreateFolder" preset="card" title="新建文件夹" class="folder-modal">
      <div class="folder-form">
        <div class="folder-parent mono" :title="createFolderParentPath()">
          {{ createFolderParentPath() }}
        </div>
        <n-input
          v-model:value="folderName"
          placeholder="文件夹名称"
          @keyup.enter="createFolder"
        />
      </div>
      <template #footer>
        <div class="folder-actions">
          <n-button @click="showCreateFolder = false">取消</n-button>
          <n-button type="primary" :disabled="!folderName.trim()" @click="createFolder">
            创建
          </n-button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.status-view {
  display: grid;
  grid-template-columns: 330px 1fr 330px;
  height: 100%;
  min-height: 0;
  background: var(--panel-bg);
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
  border-right: 1px solid var(--border);
  background: var(--panel-bg-subtle);
}
.side-pane {
  border-left: 1px solid var(--border);
  background: var(--panel-bg-subtle);
}
.list-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 40px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border);
  background: var(--toolbar-bg);
  font-size: 12px;
}
.spacer {
  flex: 1;
}
.list-scroll {
  flex: 1;
  min-height: 0;
}
.tree-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.mode-switch {
  display: flex;
  gap: 4px;
}
.group-header {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 7px 10px 5px;
  background: var(--panel-bg-muted);
  border-bottom: 1px solid var(--border-subtle);
}
.group-count {
  font-size: 11px;
  color: var(--text-muted);
}
.file-row {
  display: flex;
  gap: 6px;
  align-items: center;
  min-height: 30px;
  padding: 3px 10px 3px 12px;
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
  background: var(--panel-bg);
}
.tree-row {
  display: grid;
  grid-template-columns: 12px 16px 22px minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
  min-height: 30px;
  padding: 3px 10px;
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
  background: var(--panel-bg);
  font-size: 12px;
}
.tree-row:hover {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 54%, transparent), transparent 280px),
    var(--panel-bg-muted);
}
.tree-row.active {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 74%, transparent), transparent 320px),
    var(--accent-row);
  box-shadow: inset 3px 0 0 var(--accent);
}
.tree-caret {
  width: 0;
  height: 0;
  border-left: 4px solid transparent;
  border-right: 4px solid transparent;
  border-top: 6px solid var(--text-muted);
}
.tree-caret.collapsed {
  transform: rotate(-90deg);
}
.tree-caret.placeholder {
  border: 0;
}
.tree-icon {
  position: relative;
  display: inline-block;
  width: 15px;
  height: 15px;
}
.dir-icon {
  border-radius: 3px;
  background: linear-gradient(180deg, #ffd37a, var(--folder));
  box-shadow: inset 0 -2px 0 rgba(109, 67, 0, 0.18);
}
.dir-icon::before {
  content: '';
  position: absolute;
  left: 1px;
  top: -3px;
  width: 8px;
  height: 5px;
  border-radius: 3px 3px 0 0;
  background: #ffe0a3;
}
.file-icon {
  border: 1px solid color-mix(in srgb, var(--file) 46%, var(--border));
  border-radius: 3px;
  background:
    linear-gradient(135deg, transparent 0 72%, color-mix(in srgb, var(--file) 34%, transparent) 72%),
    var(--file-soft);
}
.file-icon::after {
  content: '';
  position: absolute;
  left: 4px;
  right: 4px;
  top: 5px;
  height: 1px;
  background: color-mix(in srgb, var(--file) 56%, var(--text-muted));
  box-shadow: 0 4px 0 color-mix(in srgb, var(--file) 56%, var(--text-muted));
  opacity: 0.55;
}
.file-row:hover {
  background: var(--panel-bg-muted);
}
.file-row.active {
  background: var(--accent-row);
  box-shadow: inset 3px 0 0 var(--accent);
}
.file-path {
  flex: 1;
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.side-tabs {
  display: flex;
  gap: 6px;
  min-height: 40px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border);
  background: var(--toolbar-bg);
  align-items: center;
}
.hint {
  font-size: 12px;
  color: var(--text-muted);
}
.folder-modal {
  width: min(520px, calc(100vw - 32px));
}
.folder-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.folder-parent {
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--panel-bg-muted);
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
  font-size: 12px;
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
