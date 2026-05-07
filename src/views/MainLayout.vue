<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { NTabs, NTabPane, NButton, NTag, NTooltip, useMessage } from 'naive-ui'

import WorkingCopyList from '../components/WorkingCopyList.vue'
import RepositoryList from '../components/RepositoryList.vue'
import StatusView from '../views/StatusView.vue'
import LogView from '../views/LogView.vue'
import CheckoutView from '../views/CheckoutView.vue'
import RemoteBrowserView from '../views/RemoteBrowserView.vue'
import EnvWarning from '../components/EnvWarning.vue'

import { useWorkingCopiesStore } from '../stores/workingCopies'
import { api, describeError } from '../api/svn'
import type { RepositoryEntry } from '../types/svn'

const wcStore = useWorkingCopiesStore()
const tab = ref<'status' | 'log' | 'remote' | 'checkout'>('status')
const message = useMessage()

const svnVersion = ref<string | null>(null)
const svnError = ref<string | null>(null)
const checkoutRepo = ref<RepositoryEntry | null>(null)
const browseRepo = ref<RepositoryEntry | null>(null)

async function detectSvn() {
  svnError.value = null
  svnVersion.value = null
  try {
    svnVersion.value = await api.checkEnvironment()
  } catch (e) {
    svnError.value = describeError(e)
  }
}

onMounted(() => {
  detectSvn()
})

const selected = computed(() => wcStore.selected)

watch(
  () => wcStore.selectedId,
  () => {
    // 切换工作副本时若当前是 checkout 页则切回 status
    if (tab.value === 'checkout' && wcStore.selectedId) {
      tab.value = 'status'
    }
  },
)

async function refreshSelected() {
  if (!selected.value) return
  try {
    await wcStore.refresh(selected.value.id)
    message.success('已刷新')
  } catch (e) {
    message.error(describeError(e))
  }
}

function checkoutRepository(repo: RepositoryEntry) {
  checkoutRepo.value = repo
  tab.value = 'checkout'
}

function browseRepository(repo: RepositoryEntry) {
  browseRepo.value = repo
  tab.value = 'remote'
}
</script>

<template>
  <div class="layout">
    <aside class="sidebar">
      <RepositoryList @checkout="checkoutRepository" @browse="browseRepository" />
      <WorkingCopyList />
    </aside>
    <main class="main">
      <header class="topbar">
        <div class="title">
          <span v-if="selected" class="path mono" :title="selected.path">
            {{ selected.path }}
          </span>
          <span v-else class="empty">未选择工作副本</span>
          <n-tag v-if="selected?.revision" size="small" type="info">r{{ selected.revision }}</n-tag>
        </div>
        <div class="actions">
          <n-button v-if="selected" size="small" @click="refreshSelected">刷新信息</n-button>
        </div>
      </header>

      <EnvWarning v-if="svnError" :message="svnError" @retry="detectSvn" />

      <n-tabs v-model:value="tab" type="line" class="tabs" pane-class="tab-pane">
        <n-tab-pane name="status" tab="状态 / 提交">
          <StatusView v-if="selected" :working-copy="selected" />
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </n-tab-pane>
        <n-tab-pane name="log" tab="历史">
          <LogView v-if="selected" :working-copy="selected" />
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </n-tab-pane>
        <n-tab-pane name="remote" tab="远端浏览">
          <RemoteBrowserView :repository="browseRepo" @checkout="checkoutRepository" />
        </n-tab-pane>
        <n-tab-pane name="checkout" tab="检出">
          <CheckoutView :repository="checkoutRepo" />
        </n-tab-pane>
      </n-tabs>

      <footer class="statusbar mono">
        <n-tooltip v-if="svnVersion">
          <template #trigger>
            <span>svn ✓ {{ svnVersion.split(/\s+/)[0] || svnVersion }}</span>
          </template>
          {{ svnVersion }}
        </n-tooltip>
        <span v-else-if="svnError" class="err">svn 不可用</span>
        <span v-else>检测 svn 中...</span>
      </footer>
    </main>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: 300px 1fr;
  height: 100vh;
  width: 100vw;
  background:
    radial-gradient(circle at 20% 0%, color-mix(in srgb, var(--accent-soft) 44%, transparent), transparent 360px),
    var(--app-bg);
  color: var(--text);
}
.sidebar {
  background: var(--sidebar-bg);
  border-right: 1px solid var(--border);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
.main {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: var(--panel-bg);
}
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.12), transparent),
    var(--toolbar-bg);
  backdrop-filter: blur(12px);
  gap: 12px;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.path {
  color: var(--text-strong);
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 60vw;
}
.empty {
  color: var(--text-muted);
}
.tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.tabs :deep(.n-tabs-nav) {
  padding: 0 14px;
  background: var(--panel-bg);
  border-bottom: 1px solid var(--border-subtle);
}
.tabs :deep(.n-tabs-tab) {
  padding: 10px 0;
}
.tabs :deep(.n-tab-pane) {
  height: 100%;
  padding: 0;
}
.tabs :deep(.n-tabs-pane-wrapper) {
  flex: 1;
  min-height: 0;
}
.tabs :deep(.tab-pane) {
  height: 100%;
}
.empty-pane {
  padding: 32px;
  text-align: center;
  color: var(--text-muted);
}
.statusbar {
  border-top: 1px solid var(--border);
  padding: 5px 14px;
  font-size: 12px;
  color: var(--text-muted);
  background: var(--panel-bg-muted);
  display: flex;
  gap: 16px;
}
.statusbar .err {
  color: #d23030;
}
</style>
