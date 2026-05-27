<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import WorkingCopyList from '../components/WorkingCopyList.vue'
import RepositoryList from '../components/RepositoryList.vue'
import StatusView from '../views/StatusView.vue'
import LogView from '../views/LogView.vue'
import CheckoutView from '../views/CheckoutView.vue'
import RemoteBrowserView from '../views/RemoteBrowserView.vue'
import EnvWarning from '../components/EnvWarning.vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { useAppToast } from '@/composables/use-app-toast'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { api, describeError } from '../api/svn'
import type { RepositoryEntry } from '../types/svn'

const wcStore = useWorkingCopiesStore()
const tab = ref<'status' | 'log' | 'remote' | 'checkout'>('status')
const toast = useAppToast()

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
    toast.success('已刷新')
  } catch (e) {
    toast.error('刷新失败', describeError(e))
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
          <Badge v-if="selected?.revision" variant="secondary">r{{ selected.revision }}</Badge>
        </div>
        <div class="actions">
          <Button v-if="selected" size="sm" variant="outline" @click="refreshSelected">刷新信息</Button>
        </div>
      </header>

      <EnvWarning v-if="svnError" :message="svnError" @retry="detectSvn" />

      <Tabs v-model="tab" class="tabs">
        <TabsList class="tabs-nav">
          <TabsTrigger value="status">状态 / 提交</TabsTrigger>
          <TabsTrigger value="log">历史</TabsTrigger>
          <TabsTrigger value="remote">远端浏览</TabsTrigger>
          <TabsTrigger value="checkout">检出</TabsTrigger>
        </TabsList>
        <TabsContent value="status" class="tab-pane">
          <StatusView v-if="selected" :working-copy="selected" />
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </TabsContent>
        <TabsContent value="log" class="tab-pane">
          <LogView v-if="selected" :working-copy="selected" />
          <div v-else class="empty-pane">先在左侧添加并选择一个工作副本</div>
        </TabsContent>
        <TabsContent value="remote" class="tab-pane">
          <RemoteBrowserView :repository="browseRepo" @checkout="checkoutRepository" />
        </TabsContent>
        <TabsContent value="checkout" class="tab-pane">
          <CheckoutView :repository="checkoutRepo" />
        </TabsContent>
      </Tabs>

      <footer class="statusbar mono">
        <TooltipProvider v-if="svnVersion">
          <Tooltip>
            <TooltipTrigger as-child>
            <span>svn ✓ {{ svnVersion.split(/\s+/)[0] || svnVersion }}</span>
            </TooltipTrigger>
            <TooltipContent>{{ svnVersion }}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        <span v-else-if="svnError" class="err">svn 不可用</span>
        <span v-else>检测 svn 中...</span>
      </footer>
    </main>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: 292px 1fr;
  height: 100vh;
  width: 100vw;
  background: var(--app-bg);
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
  backdrop-filter: blur(28px) saturate(145%);
}
.topbar {
  display: flex;
  align-items: center;
  min-height: 44px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--toolbar-bg);
  backdrop-filter: blur(26px) saturate(150%);
  gap: 12px;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}
.path {
  color: var(--text-strong);
  font-size: 12px;
  font-weight: 500;
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
.tabs-nav {
  width: 100%;
  justify-content: flex-start;
  height: 38px;
  padding: 4px 12px;
  border-radius: 0;
  background: var(--panel-bg-subtle);
  border-bottom: 1px solid var(--border-subtle);
}
.tab-pane {
  flex: 1;
  height: 100%;
  min-height: 0;
  margin: 0;
}
.tab-pane[data-state='inactive'] {
  display: none;
}
.empty-pane {
  padding: 32px;
  text-align: center;
  color: var(--text-muted);
}
.statusbar {
  border-top: 1px solid var(--border);
  min-height: 24px;
  padding: 4px 12px;
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
