<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  NButton,
  NEmpty,
  NInput,
  NInputNumber,
  NScrollbar,
  NSpin,
  NTag,
} from 'naive-ui'

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

function actionColor(a: string): 'default' | 'success' | 'info' | 'warning' | 'error' {
  switch (a) {
    case 'A':
      return 'success'
    case 'D':
      return 'error'
    case 'M':
      return 'info'
    case 'R':
      return 'warning'
    default:
      return 'default'
  }
}
</script>

<template>
  <div class="log-view">
    <section class="left">
      <div class="toolbar">
        <n-input v-model:value="search" placeholder="搜索作者/消息" size="small" />
        <n-input-number
          v-model:value="limit"
          :min="1"
          :max="500"
          size="small"
          style="width: 100px"
        />
        <n-button size="small" @click="reload">刷新</n-button>
      </div>
      <n-scrollbar class="list">
        <n-spin v-if="loading" />
        <n-empty v-else-if="entries.length === 0" description="暂无历史" size="small" />
        <div
          v-for="e in entries"
          :key="e.revision"
          :class="['rev-item', { active: selectedRev === e.revision }]"
          @click="selectedRev = e.revision"
        >
          <div class="rev-head">
            <n-tag size="small" type="info">r{{ e.revision }}</n-tag>
            <span class="author mono">{{ e.author ?? '-' }}</span>
            <span class="date">{{ formatDate(e.date) }}</span>
          </div>
          <div class="msg">{{ (e.message ?? '').split('\n')[0] || '(无消息)' }}</div>
        </div>
      </n-scrollbar>
    </section>
    <section class="right">
      <div v-if="selected" class="rev-detail">
        <div class="rev-meta">
          <n-tag size="small" type="info">r{{ selected.revision }}</n-tag>
          <span class="author mono">{{ selected.author ?? '-' }}</span>
          <span class="date">{{ formatDate(selected.date) }}</span>
        </div>
        <div class="rev-message">{{ selected.message ?? '' }}</div>
        <div class="rev-files">
          <div class="files-title">变更文件 ({{ selected.paths.length }})</div>
          <div v-for="p in selected.paths" :key="p.path" class="file-line mono">
            <n-tag size="tiny" :type="actionColor(p.action)">{{ p.action }}</n-tag>
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
  grid-template-columns: 360px 1fr;
  height: 100%;
  min-height: 0;
}
.left {
  display: flex;
  flex-direction: column;
  border-right: 1px solid rgba(127, 127, 127, 0.2);
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.2);
}
.list {
  flex: 1;
  min-height: 0;
}
.rev-item {
  padding: 6px 10px;
  cursor: pointer;
  border-bottom: 1px solid rgba(127, 127, 127, 0.1);
}
.rev-item:hover {
  background: rgba(127, 127, 127, 0.07);
}
.rev-item.active {
  background: rgba(26, 107, 255, 0.12);
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
  opacity: 0.6;
}
.msg {
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
  padding: 8px 12px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.2);
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
  opacity: 0.7;
  margin-bottom: 4px;
}
.file-line {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: 12px;
  padding: 1px 0;
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
