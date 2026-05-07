<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  NButton,
  NEmpty,
  NInput,
  NSpin,
  NTag,
  useMessage,
} from 'naive-ui'

import { api, describeError } from '../api/svn'
import type { RemoteListEntry, RepositoryEntry } from '../types/svn'

const props = defineProps<{ repository: RepositoryEntry | null }>()
const emit = defineEmits<{ checkout: [repo: RepositoryEntry] }>()

const message = useMessage()
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
    message.error(describeError(e))
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
    message.error(describeError(e))
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
          <n-tag size="small" type="info">{{ repository.name }}</n-tag>
          <span class="mono url" :title="currentUrl">{{ currentUrl }}</span>
        </div>
        <n-button size="small" @click="load()">刷新</n-button>
        <n-button size="small" type="primary" @click="checkoutCurrent">检出当前目录</n-button>
      </header>

      <div class="url-row">
        <n-input
          v-model:value="manualUrl"
          size="small"
          class="mono"
          placeholder="输入远端 SVN URL"
          @keyup.enter="load(manualUrl)"
        />
        <n-button size="small" @click="load(manualUrl)">打开</n-button>
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
            <n-spin v-if="loading" />
            <n-empty v-else-if="entries.length === 0" description="目录为空" size="small" />
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
            <n-button size="tiny" tertiary @click="closePreview">关闭</n-button>
          </div>
          <n-spin v-if="contentLoading" />
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
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--accent-soft) 28%, transparent), transparent 120px),
    var(--panel-bg);
}
.browser-toolbar,
.url-row,
.crumbs {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--border-subtle);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.1), transparent),
    var(--toolbar-bg);
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
.url-row .n-input {
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
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 34%, transparent), transparent 220px),
    var(--panel-bg-subtle);
}
.browser-body:not(.preview-open) .remote-list {
  border-right: 0;
}
.remote-table-head {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 84px 72px 110px 170px;
  gap: 10px;
  align-items: center;
  min-height: 32px;
  padding: 0 12px 0 42px;
  border-bottom: 1px solid var(--border);
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--panel-bg) 60%, transparent), transparent),
    var(--panel-bg-muted);
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
  min-height: 36px;
  padding: 3px 12px 3px 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-subtle) 78%, transparent);
  cursor: pointer;
  font-size: 12px;
  background: color-mix(in srgb, var(--panel-bg) 92%, transparent);
}
.remote-row:hover {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 60%, transparent), transparent 340px),
    var(--panel-bg-muted);
}
.remote-row.active {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 80%, transparent), transparent 420px),
    var(--accent-row);
  box-shadow: inset 3px 0 0 var(--accent);
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
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  background:
    linear-gradient(90deg, var(--file-soft), transparent 280px),
    var(--panel-bg-subtle);
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
  background:
    linear-gradient(90deg, rgba(127, 127, 127, 0.045) 1px, transparent 1px) 0 0 / 42px 42px,
    var(--panel-bg);
  color: var(--text);
}
.empty-pane,
.empty-preview {
  padding: 32px;
  color: var(--text-muted);
  text-align: center;
}
</style>
