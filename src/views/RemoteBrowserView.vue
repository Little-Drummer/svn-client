<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronRight, Folder, FileText, X } from 'lucide-vue-next'

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

      <nav class="crumbs">
        <template v-for="(crumb, idx) in crumbs" :key="crumb.url">
          <ChevronRight v-if="idx > 0" class="crumb-sep" />
          <button class="crumb" type="button" @click="load(crumb.url)">
            {{ crumb.label }}
          </button>
        </template>
      </nav>

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
                <component
                  :is="entry.kind === 'dir' ? Folder : FileText"
                  :class="['entry-icon', entry.kind === 'dir' ? 'is-dir' : 'is-file']"
                />
                <span class="name" :title="entry.url">{{ entry.name }}</span>
                <span class="size mono">{{ formatSize(entry.size) }}</span>
                <span class="rev mono">{{ entry.revision ? `r${entry.revision}` : '' }}</span>
                <span class="author mono">{{ entry.author ?? '' }}</span>
                <span class="date mono">{{ formatDate(entry.date) }}</span>
              </div>
            </template>
          </div>
        </section>

        <section v-if="previewOpen" class="preview">
          <div class="preview-head">
            <span class="mono preview-title" :title="selected?.url">{{ selected?.name ?? '文件预览' }}</span>
            <button class="preview-close" type="button" @click="closePreview">
              <X class="icon-sm" />
            </button>
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
  background: var(--mat-content);
}
.browser-toolbar,
.url-row,
.crumbs {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--mat-toolbar);
  backdrop-filter: var(--vibrancy-toolbar);
  -webkit-backdrop-filter: var(--vibrancy-toolbar);
}
.browser-toolbar {
  border-bottom: var(--hairline) solid var(--stroke-soft);
  min-height: 36px;
}
.url-row {
  padding-top: 6px;
  padding-bottom: 6px;
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
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.url-row :deep(input) {
  flex: 1;
}
.crumbs {
  overflow-x: auto;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  padding-top: 6px;
  padding-bottom: 6px;
  gap: 2px;
}
.crumb {
  border: 0;
  background: transparent;
  color: var(--accent);
  cursor: default;
  font-size: var(--fs-callout);
  font-weight: 500;
  padding: 3px 8px;
  border-radius: var(--radius-sm);
  white-space: nowrap;
  transition: background-color 120ms ease-out;
}
.crumb:hover {
  background: var(--accent-soft);
}
.crumb-sep {
  width: 11px;
  height: 11px;
  color: var(--fg-subtle);
  flex: none;
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
  background: var(--mat-content);
}
.browser-body.preview-open .remote-list {
  border-right: var(--hairline) solid var(--stroke-soft);
}
.remote-table-head {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 84px 72px 110px 170px;
  gap: 10px;
  align-items: center;
  min-height: 28px;
  padding: 0 12px 0 42px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.remote-scroll {
  flex: 1 1 auto;
  height: 0;
  min-height: 0;
  overflow-x: auto;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 4px 0;
}
.remote-row {
  display: grid;
  grid-template-columns: 16px minmax(180px, 1fr) 84px 72px 110px 170px;
  gap: 10px;
  align-items: center;
  min-height: 28px;
  padding: 3px 12px;
  margin: 1px 6px;
  border-radius: var(--radius-row);
  cursor: default;
  font-size: var(--fs-callout);
  transition: background-color 120ms ease-out;
}
.remote-row:hover {
  background: color-mix(in srgb, var(--fg) 5%, transparent);
}
.remote-row.active {
  background: var(--accent);
}
.remote-row.active .name,
.remote-row.active .size,
.remote-row.active .rev,
.remote-row.active .author,
.remote-row.active .date,
.remote-row.active .entry-icon {
  color: #fff;
}
.entry-icon {
  width: 14px;
  height: 14px;
  flex: none;
}
.entry-icon.is-dir {
  color: var(--accent);
}
.entry-icon.is-file {
  color: var(--fg-muted);
}
.name {
  color: var(--fg-strong);
  font-weight: 500;
}
.name,
.author,
.date,
.preview-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.size,
.rev,
.author,
.date {
  color: var(--fg-muted);
}
.preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 0 8px 0 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: var(--mat-toolbar);
  min-height: 32px;
  font-size: var(--fs-callout);
}
.preview-title {
  color: var(--fg-strong);
}
.preview-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: 0;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--fg-muted);
  cursor: default;
  transition: background-color 120ms ease-out, color 120ms ease-out;
}
.preview-close:hover {
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg);
}
.icon-sm {
  width: 13px;
  height: 13px;
}
.file-content {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 12px 14px;
  overflow: auto;
  font-size: var(--fs-mono);
  line-height: 1.55;
  white-space: pre-wrap;
  background: var(--mat-content);
  color: var(--fg);
}
.empty-pane,
.empty-preview {
  padding: 32px;
  color: var(--fg-muted);
  text-align: center;
  font-size: var(--fs-callout);
}
</style>
