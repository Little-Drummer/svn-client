<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  File,
  FileMinus2,
  FilePenLine,
  FilePlus2,
  FileQuestion,
  Folder,
  FolderOpen,
  RefreshCw,
  X,
} from 'lucide-vue-next'

import { api, describeError } from '../api/svn'
import type { SvnStatusEntry, UnversionedTreeEntry, WorkingCopyEntry } from '../types/svn'

const props = defineProps<{
  open: boolean
  x: number
  y: number
  workingCopy: WorkingCopyEntry | null
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const panelRef = ref<HTMLElement | null>(null)
const entries = ref<SvnStatusEntry[]>([])
const unversionedTree = ref<UnversionedTreeEntry[]>([])
const expandedPaths = ref<Set<string>>(new Set())
const loading = ref(false)
const error = ref('')
const left = ref(0)
const top = ref(0)
let requestGeneration = 0

const STATUS_ORDER = [
  'conflicted',
  'modified',
  'added',
  'deleted',
  'replaced',
  'missing',
  'obstructed',
  'unversioned',
  'incomplete',
  'normal',
]

const STATUS_META: Record<string, { label: string; tone: string; icon: typeof File }> = {
  modified: { label: '已修改', tone: 'blue', icon: FilePenLine },
  added: { label: '新增', tone: 'green', icon: FilePlus2 },
  deleted: { label: '删除', tone: 'red', icon: FileMinus2 },
  replaced: { label: '替换', tone: 'amber', icon: RefreshCw },
  missing: { label: '丢失', tone: 'amber', icon: FileQuestion },
  conflicted: { label: '冲突', tone: 'red', icon: AlertTriangle },
  unversioned: { label: '未跟踪', tone: 'gray', icon: FileQuestion },
  obstructed: { label: '阻塞', tone: 'amber', icon: AlertTriangle },
  incomplete: { label: '未完成', tone: 'amber', icon: AlertTriangle },
  normal: { label: '属性修改', tone: 'purple', icon: FilePenLine },
}

const position = computed(() => ({ left: `${left.value}px`, top: `${top.value}px` }))
const title = computed(() => props.workingCopy?.displayName || leafName(props.workingCopy?.path || ''))

type ChangeRow = {
  entry: SvnStatusEntry
  kind?: string
  depth: number
  hasChildren: boolean
}

const changeRows = computed<ChangeRow[]>(() => {
  const treeByRoot = new Map<string, UnversionedTreeEntry[]>()
  for (const item of unversionedTree.value) {
    const group = treeByRoot.get(item.rootPath) ?? []
    group.push(item)
    treeByRoot.set(item.rootPath, group)
  }

  const rows: ChangeRow[] = []
  for (const entry of entries.value) {
    const tree = treeByRoot.get(entry.path) ?? []
    const root = tree.find((item) => item.depth === 0)
    rows.push({ entry, kind: root?.kind, depth: 0, hasChildren: tree.length > 1 })
    if (entry.item !== 'unversioned') continue

    // tree 是后端按深度优先顺序返回的，用目录栈判断每一层祖先是否已展开。
    const directoryStack: string[] = []
    for (let index = 0; index < tree.length; index += 1) {
      const child = tree[index]
      directoryStack.length = child.depth
      const visible = directoryStack.every((path) => expandedPaths.value.has(path))
      const isDirectory = child.kind === 'dir'
      if (child.depth > 0 && visible) {
        rows.push({
          entry: { ...entry, path: child.path },
          kind: child.kind,
          depth: child.depth,
          hasChildren: isDirectory && (tree[index + 1]?.depth ?? 0) > child.depth,
        })
      }
      if (isDirectory) {
        directoryStack[child.depth] = child.path
      }
    }
  }
  return rows
})

function isExpanded(row: ChangeRow) {
  return expandedPaths.value.has(row.entry.path)
}

async function toggleDirectory(row: ChangeRow) {
  if (row.kind !== 'dir' || !row.hasChildren) return
  const next = new Set(expandedPaths.value)
  if (next.has(row.entry.path)) next.delete(row.entry.path)
  else next.add(row.entry.path)
  expandedPaths.value = next
  await reposition()
}

function isLocalChange(entry: SvnStatusEntry) {
  const propsChanged = !['normal', 'none'].includes(entry.props ?? 'none')
  return !['normal', 'none', 'ignored', 'external'].includes(entry.item) || propsChanged
}

function statusMeta(entry: SvnStatusEntry) {
  return STATUS_META[entry.item] ?? { label: entry.item, tone: 'gray', icon: File }
}

function relativePath(path: string) {
  const root = (props.workingCopy?.path ?? '').replace(/[\\/]+$/, '')
  if (!root || !path.startsWith(root)) return path
  return path.slice(root.length).replace(/^[\\/]+/, '') || leafName(path)
}

function leafName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

function parentPath(path: string) {
  const relative = relativePath(path)
  const parts = relative.split(/[\\/]/)
  parts.pop()
  return parts.join('/')
}

function statusRank(entry: SvnStatusEntry) {
  const index = STATUS_ORDER.indexOf(entry.item)
  return index >= 0 ? index : STATUS_ORDER.length
}

async function load() {
  const wc = props.workingCopy
  if (!wc) return
  const generation = ++requestGeneration
  loading.value = true
  error.value = ''
  unversionedTree.value = []
  expandedPaths.value = new Set()
  try {
    const result = await api.status(wc.path, true, false)
    if (generation !== requestGeneration) return
    entries.value = result.filter(isLocalChange).sort((a, b) => {
      const rank = statusRank(a) - statusRank(b)
      return rank || relativePath(a.path).localeCompare(relativePath(b.path), 'zh-CN')
    })
    const unversionedPaths = entries.value
      .filter((entry) => entry.item === 'unversioned')
      .map((entry) => entry.path)
    if (unversionedPaths.length > 0) {
      try {
        const tree = await api.listUnversionedTree(unversionedPaths)
        if (generation !== requestGeneration) return
        unversionedTree.value = tree
        // 顶层未跟踪目录默认展开一层，内部目录由用户按需继续展开。
        expandedPaths.value = new Set(
          tree
            .filter((item) => item.depth === 0 && item.kind === 'dir')
            .map((item) => item.path),
        )
      } catch {
        // 展开失败时仍保留 SVN 原始状态列表，避免辅助信息影响核心结果。
        unversionedTree.value = []
      }
    }
  } catch (cause) {
    if (generation !== requestGeneration) return
    entries.value = []
    error.value = describeError(cause)
  } finally {
    if (generation === requestGeneration) {
      loading.value = false
      // 实际内容可能远高于骨架屏，渲染完成后重新贴合视口，避免底部项目的弹窗被裁切。
      await reposition()
    }
  }
}

async function reposition() {
  left.value = props.x
  top.value = props.y
  await nextTick()
  const panel = panelRef.value
  if (!panel) return
  const { width, height } = panel.getBoundingClientRect()
  const margin = 10
  left.value = Math.max(margin, Math.min(props.x, window.innerWidth - width - margin))
  top.value = Math.max(margin, Math.min(props.y, window.innerHeight - height - margin))
}

function close() {
  emit('update:open', false)
}

function onPointerDown(event: PointerEvent) {
  if ((event.target as Element).closest('[data-local-changes-trigger]')) return
  if (panelRef.value && !panelRef.value.contains(event.target as Node)) close()
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') close()
}

function onResize() {
  close()
}

function onScroll(event: Event) {
  if (panelRef.value?.contains(event.target as Node)) return
  close()
}

function teardown() {
  document.removeEventListener('pointerdown', onPointerDown, true)
  document.removeEventListener('keydown', onKeydown, true)
  window.removeEventListener('resize', onResize, true)
  window.removeEventListener('scroll', onScroll, true)
}

watch(
  () => [props.open, props.workingCopy?.id] as const,
  ([open]) => {
    teardown()
    if (!open) return
    void load()
    void reposition()
    document.addEventListener('pointerdown', onPointerDown, true)
    document.addEventListener('keydown', onKeydown, true)
    window.addEventListener('resize', onResize, true)
    window.addEventListener('scroll', onScroll, true)
  },
)

onBeforeUnmount(() => {
  requestGeneration += 1
  teardown()
})
</script>

<template>
  <Teleport to="body">
    <Transition name="changes-popover">
      <section
        v-if="open"
        ref="panelRef"
        class="changes-panel"
        :style="position"
        role="dialog"
        aria-label="本地更改文件"
      >
        <header class="changes-header">
          <div class="header-copy">
            <div class="header-title-row">
              <span class="header-title">本地更改</span>
              <span v-if="!loading && !error" class="header-count mono">{{ entries.length }}</span>
            </div>
            <span class="header-project" :title="workingCopy?.path">{{ title }}</span>
          </div>
          <button class="icon-button" type="button" title="关闭" aria-label="关闭" @click="close">
            <X />
          </button>
        </header>

        <div class="changes-divider" />

        <div class="changes-body">
          <div v-if="loading" class="skeleton-list" aria-label="正在读取本地更改">
            <div v-for="index in 5" :key="index" class="skeleton-row">
              <span class="skeleton-icon" />
              <span class="skeleton-copy" :style="{ width: `${58 + (index % 3) * 10}%` }" />
              <span class="skeleton-status" />
            </div>
          </div>

          <div v-else-if="error" class="message-state">
            <AlertTriangle class="message-icon error-icon" />
            <strong>读取失败</strong>
            <span>{{ error }}</span>
            <button class="retry-button" type="button" @click="load">
              <RefreshCw />
              重试
            </button>
          </div>

          <div v-else-if="entries.length === 0" class="message-state">
            <File class="message-icon" />
            <strong>没有本地更改</strong>
            <span>工作副本当前是干净的</span>
          </div>

          <div v-else class="change-list">
            <div
              v-for="row in changeRows"
              :key="row.entry.path"
              :class="['change-row', { 'change-row-child': row.depth > 0 }]"
              :style="{ paddingLeft: `${7 + row.depth * 16}px` }"
              :title="row.entry.path"
            >
              <button
                v-if="row.kind === 'dir' && row.hasChildren"
                class="tree-toggle"
                type="button"
                :title="isExpanded(row) ? '收起文件夹' : '展开文件夹'"
                :aria-label="isExpanded(row) ? `收起 ${leafName(row.entry.path)}` : `展开 ${leafName(row.entry.path)}`"
                @click.stop="toggleDirectory(row)"
              >
                <ChevronDown v-if="isExpanded(row)" />
                <ChevronRight v-else />
              </button>
              <span v-else class="tree-toggle-spacer" />
              <span :class="['file-icon-wrap', `tone-${statusMeta(row.entry).tone}`]">
                <component
                  :is="row.kind === 'dir' ? (isExpanded(row) ? FolderOpen : Folder) : statusMeta(row.entry).icon"
                />
              </span>
              <div class="file-copy">
                <span class="file-name">{{ leafName(row.entry.path) }}</span>
                <span class="file-path mono">{{ parentPath(row.entry.path) || './' }}</span>
              </div>
              <span :class="['status-label', `tone-${statusMeta(row.entry).tone}`]">
                {{ statusMeta(row.entry).label }}
              </span>
            </div>
          </div>
        </div>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped>
.changes-panel {
  position: fixed;
  z-index: 1100;
  width: min(360px, calc(100vw - 20px));
  max-height: min(500px, calc(100vh - 20px));
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: var(--hairline) solid var(--stroke-strong);
  border-radius: 10px;
  color: var(--fg);
  background: var(--mat-popover);
  box-shadow: var(--shadow-pop);
  backdrop-filter: var(--vibrancy-popover);
  -webkit-backdrop-filter: var(--vibrancy-popover);
  transform-origin: left top;
}
.changes-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 12px 10px 14px;
}
.header-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.header-title-row {
  display: flex;
  align-items: center;
  gap: 7px;
}
.header-title {
  color: var(--fg-strong);
  font-size: var(--fs-body);
  font-weight: 650;
}
.header-count {
  min-width: 18px;
  height: 17px;
  padding: 0 5px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-pill);
  color: color-mix(in srgb, var(--warning) 72%, var(--fg-strong));
  background: var(--warning-soft);
  font-size: 10px;
  font-weight: 650;
}
.header-project {
  overflow: hidden;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.icon-button {
  width: 24px;
  height: 24px;
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-control);
  color: var(--fg-muted);
  background: transparent;
}
.icon-button:hover {
  color: var(--fg-strong);
  background: color-mix(in srgb, var(--fg) 7%, transparent);
}
.icon-button svg {
  width: 14px;
  height: 14px;
}
.changes-divider {
  height: var(--hairline);
  margin: 0 10px;
  flex: none;
  background: var(--stroke-soft);
}
.changes-body {
  min-height: 132px;
  overflow: auto;
  padding: 6px;
}
.change-list {
  display: flex;
  flex-direction: column;
}
.change-row {
  min-height: 43px;
  display: grid;
  grid-template-columns: 14px 26px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 5px 7px;
  border-radius: var(--radius-row);
}
.tree-toggle,
.tree-toggle-spacer {
  width: 14px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  color: var(--fg-muted);
  background: transparent;
}
.tree-toggle {
  border-radius: var(--radius-sm);
}
.tree-toggle:hover {
  color: var(--fg-strong);
  background: color-mix(in srgb, var(--fg) 7%, transparent);
}
.tree-toggle svg {
  width: 13px;
  height: 13px;
}
.change-row:hover {
  background: color-mix(in srgb, var(--fg) 5%, transparent);
}
.change-row-child {
  background: color-mix(in srgb, var(--fg) 2%, transparent);
}
.file-icon-wrap {
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-control);
  background: color-mix(in srgb, currentColor 11%, transparent);
}
.file-icon-wrap svg {
  width: 14px;
  height: 14px;
}
.file-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.file-name,
.file-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-name {
  color: var(--fg-strong);
  font-size: var(--fs-callout);
  font-weight: 550;
}
.file-path {
  color: var(--fg-subtle);
  font-size: 10px;
}
.status-label {
  height: 18px;
  display: inline-flex;
  align-items: center;
  padding: 0 6px;
  border: var(--hairline) solid color-mix(in srgb, currentColor 25%, transparent);
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, currentColor 9%, transparent);
  font-size: 10px;
  font-weight: 550;
  white-space: nowrap;
}
.tone-blue { color: var(--accent); }
.tone-green { color: var(--success); }
.tone-red { color: var(--danger); }
.tone-amber { color: var(--warning); }
.tone-purple { color: var(--purple); }
.tone-gray { color: var(--fg-muted); }
.message-state {
  min-height: 150px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 18px;
  color: var(--fg-muted);
  text-align: center;
}
.message-state strong {
  margin-top: 3px;
  color: var(--fg);
  font-size: var(--fs-callout);
  font-weight: 600;
}
.message-state span {
  max-width: 280px;
  font-size: var(--fs-caption);
  line-height: var(--lh-normal);
}
.message-icon {
  width: 22px;
  height: 22px;
  color: var(--fg-subtle);
}
.error-icon { color: var(--danger); }
.retry-button {
  height: 26px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-top: 7px;
  padding: 0 9px;
  border: var(--hairline) solid var(--stroke);
  border-radius: var(--radius-control);
  color: var(--fg);
  background: var(--mat-elevated);
  box-shadow: var(--shadow-control);
  font-size: var(--fs-caption);
}
.retry-button:hover { background: color-mix(in srgb, var(--fg) 5%, var(--mat-elevated)); }
.retry-button svg {
  width: 12px;
  height: 12px;
}
.skeleton-list {
  display: flex;
  flex-direction: column;
}
.skeleton-row {
  min-height: 43px;
  display: grid;
  grid-template-columns: 14px 26px minmax(0, 1fr) 42px;
  align-items: center;
  gap: 8px;
  padding: 5px 7px;
}
.skeleton-row::before {
  width: 14px;
  content: '';
}
.skeleton-icon,
.skeleton-copy,
.skeleton-status {
  display: block;
  background: color-mix(in srgb, var(--fg) 7%, transparent);
  animation: skeleton-pulse 1.2s ease-in-out infinite;
}
.skeleton-icon {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-control);
}
.skeleton-copy {
  height: 9px;
  border-radius: var(--radius-sm);
}
.skeleton-status {
  width: 42px;
  height: 17px;
  border-radius: var(--radius-pill);
}
.changes-popover-enter-active,
.changes-popover-leave-active {
  transition:
    opacity 130ms ease-out,
    transform 170ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.changes-popover-enter-from,
.changes-popover-leave-to {
  opacity: 0;
  transform: translateX(-5px) scale(0.97);
}
@keyframes skeleton-pulse {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
  .changes-popover-enter-active,
  .changes-popover-leave-active,
  .skeleton-icon,
  .skeleton-copy,
  .skeleton-status {
    transition: none;
    animation: none;
  }
}
</style>
