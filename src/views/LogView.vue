<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ArrowLeft, ChevronDown, ChevronRight, RefreshCw } from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import DiffViewer from '../components/DiffViewer.vue'
import { api } from '../api/svn'
import { useErrorToast } from '../composables/use-error-toast'
import { createGeneration } from '../composables/use-request-generation'
import type { LogTarget, SvnLogEntry, SvnLogPath } from '../types/svn'

const props = defineProps<{ target: LogTarget }>()
const toast = useErrorToast()

const entries = ref<SvnLogEntry[]>([])
const loading = ref(false)
const limit = ref<number>(50)
const search = ref('')
const author = ref('')
const revisionRange = ref('')
const dateFrom = ref('')
const dateTo = ref('')

// 展开的提交（展开后内联显示其改动文件）
const expanded = ref<Set<number>>(new Set())

// SVN 工作副本 revision 是仓库全局版本，日志是当前路径历史；取不大于它的最新日志作为副本位置。
const currentMarkerRevision = computed(() => {
  const current = props.target.currentRevision
  if (props.target.kind !== 'wc' || current == null) return null
  let marker: number | null = null
  for (const entry of entries.value) {
    if (entry.revision <= current && (marker == null || entry.revision > marker)) {
      marker = entry.revision
    }
  }
  return marker
})

const logGen = createGeneration()

async function reload() {
  const token = logGen.next()
  loading.value = true
  try {
    const result = await api.log({
      path: props.target.target,
      limit: limit.value || 50,
      search: search.value || undefined,
      author: author.value || undefined,
      revisionRange: revisionRange.value || undefined,
      dateFrom: dateFrom.value || undefined,
      dateTo: dateTo.value || undefined,
      withPaths: true,
    })
    if (!logGen.isCurrent(token)) return
    entries.value = result
    // 默认展开最新一条，方便一眼看到最近改了哪些文件
    const next = new Set<number>()
    if (result.length > 0) next.add(result[0].revision)
    expanded.value = next
  } catch (e) {
    if (!logGen.isCurrent(token)) return
    toast(e, '加载历史失败')
  } finally {
    if (logGen.isCurrent(token)) loading.value = false
  }
}

// 切换日志目标时回到列表态、清空筛选并重新加载
watch(
  () => props.target.target,
  () => {
    fileDiff.value = null
    search.value = ''
    author.value = ''
    revisionRange.value = ''
    dateFrom.value = ''
    dateTo.value = ''
    reload()
  },
)
onMounted(reload)

function toggle(rev: number) {
  const next = new Set(expanded.value)
  if (next.has(rev)) {
    next.delete(rev)
  } else {
    next.add(rev)
  }
  expanded.value = next
}

// ===== 单文件 diff（全屏左右对比）=====
interface FileDiffState {
  rev: number
  path: string // repo-root-relative
  name: string // 文件名，用于语言识别与头部展示
}
const fileDiff = ref<FileDiffState | null>(null)
const baseContent = ref<string | null>(null)
const currentContent = ref<string | null>(null)
const diffText = ref<string | null>(null)
const diffLoading = ref(false)
const diffGen = createGeneration()

// 把 log 给的 repo-root-relative path 拼成完整文件 URL（逐段编码，处理空格/中文）
function fileUrl(path: string): string | null {
  const root = props.target.repositoryRoot?.replace(/\/+$/, '')
  if (!root) {
    return null
  }
  const encoded = path.split('/').filter(Boolean).map(encodeURIComponent).join('/')
  return `${root}/${encoded}`
}

async function openFileDiff(rev: number, p: SvnLogPath) {
  const name = p.path.split('/').filter(Boolean).pop() ?? p.path
  const url = fileUrl(p.path)
  if (!url) {
    toast(new Error('缺少仓库根地址，无法读取文件的两个版本'), '无法打开文件对比')
    return
  }
  fileDiff.value = { rev, path: p.path, name }

  // p.path 是这条日志在 rev 时刻的路径；用 @rev 钉住 peg revision，
  // svn 才会顺着改名/移动历史去找 rev-1 的内容，否则一旦中间发生过改名，
  // 不加 peg 的写法会先按当前/HEAD 语义解析这个路径，跨改名直接报 "文件未找到"。
  const pegUrl = `${url}@${rev}`

  const token = diffGen.next()
  baseContent.value = null
  currentContent.value = null
  diffText.value = null
  diffLoading.value = true
  try {
    // 新增文件在 N-1 本就不存在、删除文件在 N 本就不存在，这两种预期内的缺失按空内容处理；
    // 其余情况读取失败要报出来，不能悄悄当成"没有改动"，否则会跟真的无变更混淆。
    const [base, cur, dt] = await Promise.all([
      api.catRevision(pegUrl, Math.max(rev - 1, 0)).catch((e) => {
        if (p.action === 'A') return ''
        throw e
      }),
      api.catRevision(pegUrl, rev).catch((e) => {
        if (p.action === 'D') return ''
        throw e
      }),
      api.diffRevision(pegUrl, rev).catch(() => ''),
    ])
    if (!diffGen.isCurrent(token)) return
    baseContent.value = base
    currentContent.value = cur
    diffText.value = dt
  } catch (e) {
    if (!diffGen.isCurrent(token)) return
    toast(e, '加载文件差异失败')
  } finally {
    if (diffGen.isCurrent(token)) diffLoading.value = false
  }
}

function backToList() {
  fileDiff.value = null
}

function formatDate(d?: string | null) {
  if (!d) return ''
  try {
    return new Date(d).toLocaleString()
  } catch {
    return d
  }
}

function firstLine(msg?: string | null) {
  return (msg ?? '').split('\n')[0] || '(无消息)'
}

function hasMoreLines(msg?: string | null) {
  return !!msg && msg.split('\n').length > 1
}

function actionClass(a: string) {
  switch (a) {
    case 'A':
      return 'pill-added'
    case 'D':
      return 'pill-deleted'
    case 'M':
      return 'pill-modified'
    case 'R':
      return 'pill-warning'
    default:
      return 'pill-muted'
  }
}
</script>

<template>
  <div class="log-view">
    <header class="log-head">
      <div class="head-title">
        <Badge variant="secondary">{{ target.kind === 'wc' ? '工作副本' : '远端' }}</Badge>
        <span class="title mono" :title="target.target">{{ target.title }}</span>
      </div>
      <div class="head-filters">
        <Input v-model="search" placeholder="关键词" class="f-search" @keyup.enter="reload" />
        <Input v-model="author" placeholder="作者" class="f-author" @keyup.enter="reload" />
        <Input v-model="revisionRange" placeholder="HEAD:1" class="f-range" @keyup.enter="reload" />
        <Input
          :model-value="limit"
          type="number"
          min="1"
          max="500"
          class="f-limit"
          @update:model-value="(v) => (limit = Number(v) || 50)"
          @keyup.enter="reload"
        />
        <Input v-model="dateFrom" placeholder="起 2026-01-01" class="f-date" @keyup.enter="reload" />
        <Input v-model="dateTo" placeholder="止 2026-05-07" class="f-date" @keyup.enter="reload" />
        <Button size="sm" variant="ghost" class="refresh-btn" @click="reload">
          <RefreshCw class="icon-sm" />
        </Button>
      </div>
    </header>

    <!-- 列表态：全宽提交列表，每条可展开看改动文件 -->
    <div v-if="!fileDiff" class="commit-list">
      <LoadingSpinner v-if="loading" />
      <EmptyState v-else-if="entries.length === 0" description="暂无历史" />
      <template v-else>
        <div v-for="e in entries" :key="e.revision" class="commit">
          <button
            type="button"
            :class="['commit-row', { current: e.revision === currentMarkerRevision }]"
            @click="toggle(e.revision)"
          >
            <component
              :is="expanded.has(e.revision) ? ChevronDown : ChevronRight"
              class="chev"
            />
            <span class="rev mono">r{{ e.revision }}</span>
            <span class="msg">{{ firstLine(e.message) }}</span>
            <span class="spacer" />
            <Badge
              v-if="e.revision === currentMarkerRevision"
              class="current-badge"
              :title="`当前副本 revision: r${target.currentRevision}`"
            >
              当前副本
            </Badge>
            <span class="author mono">{{ e.author ?? '-' }}</span>
            <span class="date mono">{{ formatDate(e.date) }}</span>
            <span class="filecount mono">{{ e.paths.length }} 文件</span>
          </button>

          <div v-if="expanded.has(e.revision)" class="commit-body">
            <div v-if="hasMoreLines(e.message)" class="full-msg">{{ e.message }}</div>
            <EmptyState v-if="e.paths.length === 0" description="该版本无文件变更" />
            <div
              v-for="p in e.paths"
              :key="p.path"
              class="file-row"
              :title="`双击查看 ${p.path} 在 r${e.revision} 的改动`"
              @dblclick="openFileDiff(e.revision, p)"
            >
              <span :class="['action-pill mono', actionClass(p.action)]">{{ p.action }}</span>
              <span class="path mono">{{ p.path }}</span>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 差异态：双击文件后整块铺满主区域 -->
    <div v-else class="file-diff">
      <div class="diff-bar">
        <Button size="sm" variant="ghost" class="back-btn" @click="backToList">
          <ArrowLeft class="icon-sm" />
          返回
        </Button>
        <span class="diff-path mono" :title="fileDiff.path">{{ fileDiff.path }}</span>
        <Badge variant="default" class="mono">r{{ fileDiff.rev }}</Badge>
      </div>
      <div class="diff-host">
        <DiffViewer
          :key="`${fileDiff.rev}:${fileDiff.path}`"
          :diff-text="diffText"
          :base-content="baseContent"
          :current-content="currentContent"
          :filename="fileDiff.name"
          :loading="diffLoading"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.log-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
}

/* ===== 头部：标题 + 筛选 ===== */
.log-head {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
}
.head-title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.head-title .title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.head-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.f-search { width: 180px; }
.f-author { width: 120px; }
.f-range { width: 96px; }
.f-limit { width: 72px; }
.f-date { width: 130px; }
.refresh-btn { flex: none; }
.icon-sm {
  width: 13px;
  height: 13px;
}

/* ===== 提交列表 ===== */
.commit-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 0 12px;
}
.commit {
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.commit-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 14px;
  border: 0;
  background: transparent;
  color: var(--fg);
  text-align: left;
  font: inherit;
  cursor: default;
  transition: background-color 120ms ease-out;
}
.commit-row:hover {
  background: color-mix(in srgb, var(--fg) 4%, transparent);
}
/* 当前工作副本所在版本：左侧 accent 条 + 轻底色，一眼可辨 */
.commit-row.current {
  background: var(--accent-soft);
  box-shadow: inset 3px 0 0 var(--accent);
}
.chev {
  width: 14px;
  height: 14px;
  flex: none;
  color: var(--fg-muted);
}
.rev {
  flex: none;
  font-weight: 600;
  font-size: var(--fs-callout);
  color: var(--accent);
  font-feature-settings: 'tnum';
}
.msg {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-body);
  color: var(--fg-strong);
}
.spacer {
  flex: 1;
}
.current-badge {
  flex: none;
  height: 18px;
  padding: 0 7px;
  font-size: 10px;
  font-weight: 600;
  border-radius: var(--radius-pill);
  background: var(--accent);
  color: #fff;
  border: 0;
}
.commit-row .author {
  flex: none;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
}
.commit-row .date {
  flex: none;
  color: var(--fg-subtle);
  font-size: var(--fs-caption);
}
.filecount {
  flex: none;
  color: var(--fg-subtle);
  font-size: var(--fs-caption);
  font-feature-settings: 'tnum';
}

/* ===== 展开区：完整消息 + 文件列表 ===== */
.commit-body {
  padding: 2px 14px 10px 38px;
  background: color-mix(in srgb, var(--fg) 2%, transparent);
}
.full-msg {
  margin: 4px 0 8px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: var(--mat-content);
  border: var(--hairline) solid var(--stroke-soft);
  white-space: pre-wrap;
  word-break: break-word;
  font-size: var(--fs-callout);
  color: var(--fg);
  line-height: 1.5;
}
.file-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 6px;
  border-radius: var(--radius-sm);
  cursor: default;
  transition: background-color 120ms ease-out;
}
.file-row:hover {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.action-pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: var(--radius-sm);
  font-size: 10px;
  font-weight: 600;
  flex: none;
  border: var(--hairline) solid transparent;
}
.pill-added {
  background: color-mix(in srgb, var(--success) 14%, transparent);
  color: var(--success);
  border-color: color-mix(in srgb, var(--success) 28%, transparent);
}
.pill-deleted {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 28%, transparent);
}
.pill-modified {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 28%, transparent);
}
.pill-warning {
  background: color-mix(in srgb, var(--warning) 14%, transparent);
  color: var(--warning);
  border-color: color-mix(in srgb, var(--warning) 28%, transparent);
}
.pill-muted {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg-muted);
  border-color: var(--stroke-soft);
}
.file-row .path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--fs-callout);
  color: var(--fg);
}

/* ===== 文件差异态 ===== */
.file-diff {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.diff-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
}
.back-btn {
  flex: none;
  gap: 4px;
}
.diff-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
  font-size: var(--fs-callout);
  color: var(--fg-strong);
}
.diff-host {
  flex: 1;
  min-height: 0;
}
</style>
