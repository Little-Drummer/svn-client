<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, ref } from 'vue'
import {
  ChevronDown,
  FolderGit2,
  HardDrive,
  Plus,
  RefreshCw,
  Trash2,
} from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useRepositoriesStore } from '../stores/repositories'
import { useErrorToast } from '../composables/use-error-toast'

const props = withDefaults(defineProps<{ showActive?: boolean }>(), { showActive: true })

const emit = defineEmits<{
  select: [id: string]
}>()

const store = useWorkingCopiesStore()
const repoStore = useRepositoriesStore()
const toast = useErrorToast()

const items = computed(() => store.items)
const collapsedRoots = ref<Set<string>>(new Set())

function normalizeUrl(u: string) {
  return u.replace(/\/+$/, '')
}

// 若该远端 root 命中已添加的仓库配置（互为前缀视为同源），用用户起的仓库名做分组标题
function repoNameForRoot(root: string): string | null {
  const r = normalizeUrl(root)
  for (const repo of repoStore.items) {
    const u = normalizeUrl(repo.url)
    if (u === r || u.startsWith(`${r}/`) || r.startsWith(`${u}/`)) {
      return repo.name
    }
  }
  return null
}

const groups = computed(() => {
  const map = new Map<string, { root: string; copies: typeof store.items }>()
  for (const wc of store.items) {
    const root = wc.repositoryRoot ?? wc.url ?? '未知远端'
    if (!map.has(root)) map.set(root, { root, copies: [] })
    map.get(root)!.copies.push(wc)
  }
  return [...map.values()]
    .map((g) => ({ ...g, label: repoNameForRoot(g.root) ?? rootLabel(g.root) }))
    .sort((a, b) => a.label.localeCompare(b.label))
})

onMounted(() => {
  store.reload().catch(toast)
})

async function pickAndAdd() {
  try {
    const dir = await open({ directory: true, multiple: false, title: '选择 SVN 工作副本目录' })
    if (!dir || typeof dir !== 'string') return
    await store.add(dir)
  } catch (e) {
    toast(e, '添加工作副本失败')
  }
}

async function refresh(id: string) {
  try {
    await store.refresh(id)
  } catch (e) {
    toast(e, '刷新失败')
  }
}

async function remove(id: string) {
  const ok = await confirm({
    title: '移除工作副本',
    content: '移除这个工作副本？只是从列表里去掉，不会删除磁盘文件。',
    confirmText: '移除',
    destructive: true,
  })
  if (!ok) return
  try {
    await store.remove(id)
  } catch (e) {
    toast(e, '移除失败')
  }
}

function shortPath(p: string) {
  const parts = p.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || p
}

function rootLabel(root: string) {
  try {
    const u = new URL(root)
    return u.pathname.split('/').filter(Boolean).pop() || u.host
  } catch {
    const parts = root.split(/[\\/]/).filter(Boolean)
    return parts[parts.length - 1] || root
  }
}

function selectWc(id: string) {
  store.select(id)
  emit('select', id)
}

function toggleRoot(root: string) {
  const next = new Set(collapsedRoots.value)
  if (next.has(root)) next.delete(root)
  else next.add(root)
  collapsedRoots.value = next
}
</script>

<template>
  <div class="wc-list">
    <div class="wc-section-head">
      <span class="section-title">工作副本</span>
      <Button size="xs" variant="ghost" class="head-action" @click="pickAndAdd">
        <Plus class="icon-xs" />
        添加
      </Button>
    </div>
    <div class="wc-scroll">
      <div v-if="items.length === 0" class="wc-empty">
        <EmptyState description="还没有工作副本，点上面按钮选一个本地目录" />
      </div>
      <div v-for="group in groups" :key="group.root" class="wc-group">
        <button class="root-row" type="button" @click="toggleRoot(group.root)">
          <ChevronDown
            :class="['root-chevron', { collapsed: collapsedRoots.has(group.root) }]"
          />
          <FolderGit2 class="root-icon" />
          <span class="root-name" :title="group.root">{{ group.label }}</span>
          <span class="root-count mono">{{ group.copies.length }}</span>
        </button>
        <div v-if="!collapsedRoots.has(group.root)" class="copy-list">
          <div
            v-for="wc in group.copies"
            :key="wc.id"
            :class="['wc-item', { active: props.showActive && wc.id === store.selectedId }]"
            @click="selectWc(wc.id)"
          >
            <div class="wc-row-main">
              <HardDrive class="wc-icon" />
              <div class="wc-text">
                <div class="wc-name" :title="wc.path">{{ shortPath(wc.path) }}</div>
                <div class="wc-meta">
                  <span v-if="wc.revision" class="meta-rev mono">r{{ wc.revision }}</span>
                  <span v-if="wc.url" class="wc-url mono" :title="wc.url">{{ wc.url }}</span>
                </div>
              </div>
              <div class="wc-actions" @click.stop>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        class="row-icon-btn"
                        @click="refresh(wc.id)"
                      >
                        <RefreshCw class="icon-xs" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>重新读取 svn info</TooltipContent>
                  </Tooltip>
                </TooltipProvider>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        class="row-icon-btn danger-action"
                        @click="remove(wc.id)"
                      >
                        <Trash2 class="icon-xs" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>从列表移除</TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wc-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: transparent;
}

/* ===== 分组标题（NSSourceList 风格：小写灰、字距）===== */
.wc-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 14px 6px 14px;
  user-select: none;
}
.section-title {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--fg-muted);
}
.head-action {
  height: 22px;
  padding: 0 6px;
  color: var(--fg-muted);
  font-size: var(--fs-caption);
  gap: 4px;
}
.head-action:hover {
  color: var(--fg-strong);
}

.wc-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow: auto;
  padding: 2px 0 8px;
}
.wc-empty {
  padding: 16px 12px;
  text-align: center;
}

/* ===== 仓库分组 ===== */
.wc-group {
  margin-bottom: 4px;
}
.root-row {
  width: calc(100% - 12px);
  margin: 0 6px 1px;
  min-height: 26px;
  border: 0;
  border-radius: var(--radius-row);
  background: transparent;
  color: var(--fg);
  display: grid;
  grid-template-columns: 14px 16px minmax(0, 1fr) auto;
  gap: 7px;
  align-items: center;
  cursor: pointer;
  text-align: left;
  font: inherit;
  padding: 3px 8px;
  transition: background-color 120ms ease-out;
}
.root-row:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.root-chevron {
  width: 12px;
  height: 12px;
  color: var(--fg-muted);
  transition: transform 140ms ease-out;
}
.root-chevron.collapsed {
  transform: rotate(-90deg);
}
.root-icon {
  width: 14px;
  height: 14px;
  color: var(--accent);
  flex: none;
}
.root-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
  color: var(--fg-strong);
  font-size: var(--fs-callout);
}
.root-count {
  color: var(--fg-subtle);
  font-size: var(--fs-caption);
  font-feature-settings: 'tnum';
}

/* ===== 工作副本行（NSSourceList capsule selection）===== */
.copy-list {
  padding-left: 18px;
}
.wc-item {
  margin: 1px 6px;
  padding: 0;
  border-radius: var(--radius-row);
  background: transparent;
  cursor: pointer;
  transition: background-color 120ms ease-out;
  position: relative;
}
.wc-item:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.wc-item.active {
  background: var(--accent);
  color: var(--fg-on-accent);
}
.wc-row-main {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  min-height: 36px;
}
.wc-icon {
  width: 14px;
  height: 14px;
  color: var(--fg-muted);
  flex: none;
}
.wc-item.active .wc-icon {
  color: rgba(255, 255, 255, 0.85);
}
.wc-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.wc-name {
  font-size: var(--fs-callout);
  font-weight: 500;
  color: var(--fg-strong);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-item.active .wc-name {
  color: #fff;
  font-weight: 600;
}
.wc-meta {
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}
.wc-item.active .wc-meta {
  color: rgba(255, 255, 255, 0.75);
}
.meta-rev {
  flex: none;
  font-feature-settings: 'tnum';
}
.wc-url {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 140ms ease-out;
}
.wc-item:hover .wc-actions,
.wc-item.active .wc-actions {
  opacity: 1;
}
.row-icon-btn {
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--fg-muted);
}
.row-icon-btn:hover {
  color: var(--fg-strong);
  background: color-mix(in srgb, var(--fg) 8%, transparent);
}
.wc-item.active .row-icon-btn {
  color: rgba(255, 255, 255, 0.78);
}
.wc-item.active .row-icon-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.16);
}
.danger-action:hover {
  color: var(--danger) !important;
}
.wc-item.active .danger-action:hover {
  color: #ffd6d3 !important;
}

.icon-xs {
  width: 12px;
  height: 12px;
}
</style>
