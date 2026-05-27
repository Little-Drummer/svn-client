<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import { useAppToast } from '@/composables/use-app-toast'
import { api, describeError } from '../api/svn'
import type { RemoteListEntry, RepositoryEntry } from '../types/svn'

const props = defineProps<{ repository: RepositoryEntry | null }>()
const emit = defineEmits<{ checkout: [repo: RepositoryEntry] }>()

const toast = useAppToast()
const currentUrl = ref('')
const entries = ref<RemoteListEntry[]>([])
const selected = ref<RemoteListEntry | null>(null)
const loading = ref(false)
const contentLoading = ref(false)
const fileContent = ref('')
const manualUrl = ref('')
const previewOpen = ref(false)

const repoRoot = computed(() => props.repository?.url.trim().replace(/\/+$/, '') ?? '')

const crumbs = computed(() => {
  if (!repoRoot.value || !currentUrl.value.startsWith(repoRoot.value)) return []
  const rest = currentUrl.value.slice(repoRoot.value.length).replace(/^\/+/, '')
  const parts = rest ? rest.split('/').filter(Boolean) : []
  const result = [{ label: props.repository?.name ?? '仓库', url: repoRoot.value }]
  let url = repoRoot.value
  for (const part of parts) {
    url = `${url}/${part}`
    result.push({ label: decodeURIComponent(part), url })
  }
  return result
})

function formatSize(size?: number | null) {
  if (!size) return ''
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
}

function formatDate(value?: string | null) {
  if (!value) return ''
  try {
    return new Date(value).toLocaleString()
  } catch {
    return value
  }
}

async function load(url = currentUrl.value) {
  if (!props.repository || !url.trim()) return
  loading.value = true
  selected.value = null
  fileContent.value = ''
  previewOpen.value = false
  try {
    currentUrl.value = url.trim().replace(/\/+$/, '')
    manualUrl.value = currentUrl.value
    entries.value = await api.listRemote({
      url: currentUrl.value,
      username: props.repository.username ?? undefined,
    })
  } catch (e) {
    entries.value = []
    toast.error('加载远端目录失败', describeError(e))
  } finally {
    loading.value = false
  }
}

async function selectEntry(entry: RemoteListEntry) {
  selected.value = entry
  if (entry.kind === 'dir') {
    await load(entry.url)
  }
}

async function previewEntry(entry: RemoteListEntry) {
  if (entry.kind === 'dir') {
    await load(entry.url)
    return
  }
  selected.value = entry
  previewOpen.value = true
  fileContent.value = ''
  contentLoading.value = true
  try {
    fileContent.value = await api.catRemote({
      url: entry.url,
      username: props.repository?.username ?? undefined,
    })
  } catch (e) {
    toast.error('加载文件失败', describeError(e))
  } finally {
    contentLoading.value = false
  }
}

function closePreview() {
  previewOpen.value = false
  fileContent.value = ''
}

function checkoutCurrent() {
  if (!props.repository) return
  emit('checkout', {
    ...props.repository,
    url: selected.value?.kind === 'dir' ? selected.value.url : currentUrl.value,
  })
}

watch(
  () => props.repository?.id,
  () => {
    if (!props.repository) return
    currentUrl.value = repoRoot.value
    manualUrl.value = repoRoot.value
    load(repoRoot.value)
  },
  { immediate: true },
)
</script>

<template>
  <div class="remote-browser">
    <div v-if="!repository" class="empty-pane">先在左侧选择一个远端仓库</div>
    <template v-else>
      <header class="browser-toolbar">
        <div class="repo-title">
          <Badge variant="secondary">{{ repository.name }}</Badge>
          <span class="mono url" :title="currentUrl">{{ currentUrl }}</span>
        </div>
        <Button size="sm" variant="outline" @click="load()">刷新</Button>
        <Button size="sm" @click="checkoutCurrent">检出当前目录</Button>
      </header>

      <div class="url-row">
        <Input
          v-model="manualUrl"
          class="mono"
          placeholder="输入远端 SVN URL"
          @keyup.enter="load(manualUrl)"
        />
        <Button size="sm" variant="outline" @click="load(manualUrl)">打开</Button>
      </div>

      <div class="crumbs">
        <button
          v-for="crumb in crumbs"
          :key="crumb.url"
          class="crumb"
          type="button"
          @click="load(crumb.url)"
        >
          {{ crumb.label }}
        </button>
      </div>

      <div :class="['browser-body', { 'preview-open': previewOpen }]">
        <section class="remote-list">
          <div class="remote-table-head">
            <span>名称</span>
            <span>大小</span>
            <span>版本</span>
            <span>作者</span>
            <span>时间</span>
          </div>
          <div class="remote-scroll">
            <LoadingSpinner v-if="loading" />
            <EmptyState v-else-if="entries.length === 0" description="目录为空" />
            <template v-else>
              <div
                v-for="entry in entries"
                :key="entry.url"
                :class="['remote-row', { active: selected?.url === entry.url }]"
                @click="selectEntry(entry)"
                @dblclick="previewEntry(entry)"
              >
                <span :class="['entry-icon', entry.kind === 'dir' ? 'dir-icon' : 'file-icon']" />
                <span class="name mono" :title="entry.url">{{ entry.name }}</span>
                <span class="size mono">{{ formatSize(entry.size) }}</span>
                <span class="rev mono" v-if="entry.revision">r{{ entry.revision }}</span>
                <span v-else class="rev mono" />
                <span class="author mono">{{ entry.author ?? '' }}</span>
                <span class="date">{{ formatDate(entry.date) }}</span>
              </div>
            </template>
          </div>
        </section>

        <section v-if="previewOpen" class="preview">
          <div class="preview-head">
            <span class="mono" :title="selected?.url">{{ selected?.name ?? '文件预览' }}</span>
            <Button size="xs" variant="ghost" @click="closePreview">关闭</Button>
          </div>
          <LoadingSpinner v-if="contentLoading" />
          <pre v-else-if="fileContent" class="file-content mono">{{ fileContent }}</pre>
          <div v-else class="empty-preview">文件内容为空</div>
        </section>
      </div>
    </template>
  </div>
</template>

<style scoped>
.remote-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--panel-bg);
}
.browser-toolbar,
.url-row,
.crumbs {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--toolbar-bg);
}
.repo-title {
  display: flex;
  gap: 8px;
  min-width: 0;
  flex: 1;
  align-items: center;
}
.url {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--text-muted);
}
.url-row input {
  flex: 1;
}
.crumbs {
  overflow-x: auto;
}
.crumb {
  border: 0;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  font-size: 12px;
  padding: 3px 4px;
  border-radius: 5px;
}
.crumb:hover {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.crumb:not(:last-child)::after {
  content: '/';
  margin-left: 8px;
  opacity: 0.45;
}
.browser-body {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  flex: 1;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
.browser-body.preview-open {
  grid-template-columns: minmax(460px, 56%) minmax(360px, 44%);
}
.remote-list,
.preview {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.remote-list {
  border-right: 1px solid var(--border);
  background: var(--panel-bg-subtle);
}
.browser-body:not(.preview-open) .remote-list {
  border-right: 0;
}
.remote-table-head {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 84px 72px 110px 170px;
  gap: 10px;
  align-items: center;
  min-height: 30px;
  padding: 0 12px 0 42px;
  border-bottom: 1px solid var(--border);
  background: var(--panel-bg-muted);
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}
.remote-scroll {
  flex: 1 1 auto;
  height: 0;
  min-height: 0;
  overflow-x: auto;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.remote-row {
  display: grid;
  grid-template-columns: 22px minmax(180px, 1fr) 84px 72px 110px 170px;
  gap: 10px;
  align-items: center;
  min-height: 32px;
  padding: 3px 12px 3px 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-subtle) 78%, transparent);
  cursor: pointer;
  font-size: 12px;
  background: var(--panel-bg);
}
.remote-row:hover {
  background: var(--panel-bg-muted);
}
.remote-row.active {
  background: var(--accent-row);
}
.entry-icon {
  position: relative;
  display: inline-block;
  width: 16px;
  height: 16px;
  flex: none;
}
.dir-icon {
  border-radius: 3px;
  background: color-mix(in srgb, var(--folder) 78%, white);
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
  background: var(--file-soft);
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
.name {
  color: var(--text-strong);
}
.name,
.author,
.date,
.preview-head span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.size,
.rev,
.author,
.date {
  opacity: 0.65;
  color: var(--text-muted);
}
.preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--panel-bg-subtle);
  min-height: 32px;
}
.file-content {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 10px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  background: var(--panel-bg);
  color: var(--text);
}
.empty-pane,
.empty-preview {
  padding: 32px;
  color: var(--text-muted);
  text-align: center;
}
</style>
