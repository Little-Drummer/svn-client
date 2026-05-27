<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import DiffViewer from '../components/DiffViewer.vue'
import { api } from '../api/svn'
import { useErrorToast } from '../composables/use-error-toast'
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

async function reload() {
  loading.value = true
  try {
    entries.value = await api.log({
      path: props.workingCopy.path,
      limit: limit.value || 50,
      search: search.value || undefined,
      author: author.value || undefined,
      revisionRange: revisionRange.value || undefined,
      dateFrom: dateFrom.value || undefined,
      dateTo: dateTo.value || undefined,
      withPaths: true,
    })
    if (entries.value.length > 0 && selectedRev.value == null) {
      selectedRev.value = entries.value[0].revision
    }
  } catch (e) {
    toast(e, '加载历史失败')
  } finally {
    loading.value = false
  }
}

watch(
  () => props.workingCopy.id,
  () => {
    selectedRev.value = null
    diffText.value = null
    reload()
  },
)

onMounted(reload)

watch(selectedRev, async (rev) => {
  diffText.value = null
  if (rev == null) return
  diffLoading.value = true
  try {
    diffText.value = await api.diffRevision(props.workingCopy.path, rev)
  } catch (e) {
    toast(e, '加载 revision diff 失败')
  } finally {
    diffLoading.value = false
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
      return 'status-added'
    case 'D':
      return 'status-deleted'
    case 'M':
      return 'status-modified'
    case 'R':
      return 'status-warning'
    default:
      return ''
  }
}
</script>

<template>
  <div class="log-view">
    <section class="left">
      <div class="toolbar">
        <Input v-model="search" placeholder="关键词" class="h-8" />
        <Input v-model="author" placeholder="作者" class="h-8" />
        <Input
          :model-value="limit"
          type="number"
          min="1"
          max="500"
          class="h-8 limit-input"
          @update:model-value="(v) => (limit = Number(v) || 50)"
        />
        <Input v-model="revisionRange" placeholder="HEAD:1" class="h-8" />
        <Button size="sm" variant="outline" @click="reload">刷新</Button>
      </div>
      <div class="toolbar secondary">
        <Input v-model="dateFrom" placeholder="开始日期 2026-01-01" class="h-8" />
        <Input v-model="dateTo" placeholder="结束日期 2026-05-07" class="h-8" />
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
            <Badge variant="secondary">r{{ e.revision }}</Badge>
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
          <Badge variant="secondary">r{{ selected.revision }}</Badge>
          <span class="author mono">{{ selected.author ?? '-' }}</span>
          <span class="date">{{ formatDate(selected.date) }}</span>
        </div>
        <div class="rev-message">{{ selected.message ?? '' }}</div>
        <div class="rev-files">
          <div class="files-title">变更文件 ({{ selected.paths.length }})</div>
          <div v-for="p in selected.paths" :key="p.path" class="file-line mono">
            <Badge variant="outline" :class="actionClass(p.action)">{{ p.action }}</Badge>
            <span class="path">{{ p.path }}</span>
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
  background: var(--panel-bg);
}
.left {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border);
  background: var(--panel-bg-subtle);
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 6px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--toolbar-bg);
}
.toolbar.secondary {
  padding-top: 0;
}
.list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.limit-input {
  width: 100px;
}
.rev-item {
  padding: 7px 10px;
  cursor: pointer;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--panel-bg);
}
.rev-item:hover {
  background: var(--panel-bg-muted);
}
.rev-item.active {
  background: var(--accent-row);
}
.rev-head {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: 11px;
}
.author {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.date {
  color: var(--text-muted);
}
.msg {
  color: var(--text-strong);
  font-size: 13px;
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
  padding: 9px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--panel-bg-subtle);
  max-height: 40%;
  overflow: auto;
}
.rev-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 12px;
}
.rev-message {
  margin-top: 6px;
  white-space: pre-wrap;
  font-size: 13px;
}
.rev-files {
  margin-top: 8px;
}
.files-title {
  font-size: 12px;
  color: var(--text-muted);
  margin-bottom: 4px;
}
.file-line {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: 12px;
  padding: 1px 0;
}
.status-added {
  color: var(--success);
}
.status-deleted {
  color: var(--destructive);
}
.status-modified {
  color: var(--accent);
}
.status-warning {
  color: var(--warning);
}
.path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rev-diff {
  min-height: 0;
}
</style>
