<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RefreshCw } from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import DiffViewer from '../components/DiffViewer.vue'
import { api } from '../api/svn'
import { useErrorToast } from '../composables/use-error-toast'
import { createGeneration } from '../composables/use-request-generation'
import type { SvnLogEntry, WorkingCopyEntry } from '../types/svn'

const props = defineProps<{ workingCopy: WorkingCopyEntry }>()
const toast = useErrorToast()

const entries = ref<SvnLogEntry[]>([])
const loading = ref(false)
const limit = ref<number>(50)
const search = ref('')
const author = ref('')
const revisionRange = ref('')
const dateFrom = ref('')
const dateTo = ref('')

const selectedRev = ref<number | null>(null)
const diffText = ref<string | null>(null)
const diffLoading = ref(false)

const selected = computed(() =>
  selectedRev.value != null ? entries.value.find((e) => e.revision === selectedRev.value) : null,
)

const logGen = createGeneration()

async function reload() {
  const token = logGen.next()
  loading.value = true
  try {
    const result = await api.log({
      path: props.workingCopy.path,
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
    if (entries.value.length > 0 && selectedRev.value == null) {
      selectedRev.value = entries.value[0].revision
    }
  } catch (e) {
    if (!logGen.isCurrent(token)) return
    toast(e, '加载历史失败')
  } finally {
    if (logGen.isCurrent(token)) loading.value = false
  }
}

// 按 WC.id 暂存查询条件与选中，切回时恢复
type LogViewState = {
  selectedRev: number | null
  search: string
  author: string
  revisionRange: string
  dateFrom: string
  dateTo: string
  limit: number
}
const wcLogState = new Map<string, LogViewState>()

function snapshotLogState(id: string) {
  wcLogState.set(id, {
    selectedRev: selectedRev.value,
    search: search.value,
    author: author.value,
    revisionRange: revisionRange.value,
    dateFrom: dateFrom.value,
    dateTo: dateTo.value,
    limit: limit.value,
  })
}

function restoreLogState(id: string) {
  const saved = wcLogState.get(id)
  if (saved) {
    selectedRev.value = saved.selectedRev
    search.value = saved.search
    author.value = saved.author
    revisionRange.value = saved.revisionRange
    dateFrom.value = saved.dateFrom
    dateTo.value = saved.dateTo
    limit.value = saved.limit
  } else {
    selectedRev.value = null
    search.value = ''
    author.value = ''
    revisionRange.value = ''
    dateFrom.value = ''
    dateTo.value = ''
    limit.value = 50
  }
  diffText.value = null
}

watch(
  () => props.workingCopy.id,
  (newId, oldId) => {
    if (oldId) snapshotLogState(oldId)
    restoreLogState(newId)
    reload()
  },
)

onMounted(reload)

const revDiffGen = createGeneration()

watch(selectedRev, async (rev) => {
  const token = revDiffGen.next()
  diffText.value = null
  if (rev == null) return
  diffLoading.value = true
  try {
    const result = await api.diffRevision(props.workingCopy.path, rev)
    if (!revDiffGen.isCurrent(token)) return
    diffText.value = result
  } catch (e) {
    if (!revDiffGen.isCurrent(token)) return
    toast(e, '加载 revision diff 失败')
  } finally {
    if (revDiffGen.isCurrent(token)) diffLoading.value = false
  }
})

function formatDate(d?: string | null) {
  if (!d) return ''
  try {
    return new Date(d).toLocaleString()
  } catch {
    return d
  }
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
    <section class="left">
      <div class="toolbar">
        <Input v-model="search" placeholder="关键词" />
        <Input v-model="author" placeholder="作者" />
        <Input
          :model-value="limit"
          type="number"
          min="1"
          max="500"
          class="limit-input"
          @update:model-value="(v) => (limit = Number(v) || 50)"
        />
        <Input v-model="revisionRange" placeholder="HEAD:1" />
        <Button size="sm" variant="ghost" class="refresh-btn" @click="reload">
          <RefreshCw class="icon-sm" />
        </Button>
      </div>
      <div class="toolbar secondary">
        <Input v-model="dateFrom" placeholder="开始日期 2026-01-01" />
        <Input v-model="dateTo" placeholder="结束日期 2026-05-07" />
      </div>
      <div class="list">
        <LoadingSpinner v-if="loading" />
        <EmptyState v-else-if="entries.length === 0" description="暂无历史" />
        <div
          v-for="e in entries"
          :key="e.revision"
          :class="['rev-item', { active: selectedRev === e.revision }]"
          @click="selectedRev = e.revision"
        >
          <div class="rev-head">
            <span class="rev-badge mono">r{{ e.revision }}</span>
            <span class="author mono">{{ e.author ?? '-' }}</span>
            <span class="date">{{ formatDate(e.date) }}</span>
          </div>
          <div class="msg">{{ (e.message ?? '').split('\n')[0] || '(无消息)' }}</div>
        </div>
      </div>
    </section>
    <section class="right">
      <div v-if="selected" class="rev-detail">
        <div class="rev-meta">
          <Badge variant="default">r{{ selected.revision }}</Badge>
          <span class="author mono">{{ selected.author ?? '-' }}</span>
          <span class="date">{{ formatDate(selected.date) }}</span>
        </div>
        <div class="rev-message">{{ selected.message ?? '' }}</div>
        <div class="rev-files">
          <div class="files-title">变更文件 ({{ selected.paths.length }})</div>
          <div v-for="p in selected.paths" :key="p.path" class="file-line">
            <span :class="['action-pill mono', actionClass(p.action)]">{{ p.action }}</span>
            <span class="path mono">{{ p.path }}</span>
          </div>
        </div>
      </div>
      <div class="rev-diff">
        <DiffViewer
          :diff-text="diffText"
          :filename="selected ? `r${selected.revision}` : null"
          :loading="diffLoading"
        />
      </div>
    </section>
  </div>
</template>

<style scoped>
.log-view {
  display: grid;
  grid-template-columns: 380px 1fr;
  height: 100%;
  min-height: 0;
  background: var(--mat-content);
}
.left {
  display: flex;
  flex-direction: column;
  border-right: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-content);
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 6px;
  padding: 8px 10px;
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
}
.toolbar:not(.secondary) {
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.toolbar.secondary {
  padding-top: 0;
  padding-bottom: 8px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.refresh-btn {
  flex: none;
}
.icon-sm {
  width: 13px;
  height: 13px;
}
.list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 0;
}
.limit-input {
  width: 80px;
  flex: none;
}
.rev-item {
  padding: 7px 12px;
  margin: 1px 6px;
  border-radius: var(--radius-row);
  cursor: default;
  transition: background-color 120ms ease-out;
}
.rev-item:hover {
  background: color-mix(in srgb, var(--fg) 5%, transparent);
}
.rev-item.active {
  background: var(--accent);
}
.rev-item.active .author,
.rev-item.active .date,
.rev-item.active .msg,
.rev-item.active .rev-badge {
  color: #fff;
}
.rev-item.active .rev-badge {
  background: rgba(255, 255, 255, 0.22);
  border-color: rgba(255, 255, 255, 0.3);
}
.rev-head {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: var(--fs-caption);
}
.rev-badge {
  font-size: var(--fs-caption);
  font-weight: 500;
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  background: var(--accent-soft);
  color: var(--accent);
  border: var(--hairline) solid color-mix(in srgb, var(--accent) 28%, transparent);
  line-height: 1.3;
}
.author {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--fg-muted);
}
.date {
  color: var(--fg-subtle);
}
.msg {
  color: var(--fg-strong);
  font-size: var(--fs-body);
  margin-top: 3px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.right {
  display: grid;
  grid-template-rows: auto 1fr;
  min-height: 0;
}
.rev-detail {
  padding: 12px 16px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-content);
  max-height: 40%;
  overflow: auto;
}
.rev-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.rev-message {
  margin-top: 8px;
  white-space: pre-wrap;
  font-size: var(--fs-body);
  color: var(--fg);
  line-height: 1.5;
}
.rev-files {
  margin-top: 12px;
}
.files-title {
  font-size: var(--fs-caption);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--fg-muted);
  margin-bottom: 6px;
}
.file-line {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: var(--fs-callout);
  padding: 2px 0;
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
.path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--fg);
}
.rev-diff {
  min-height: 0;
}
</style>
