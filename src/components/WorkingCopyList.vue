<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, ref } from 'vue'

import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useErrorToast } from '../composables/use-error-toast'

const store = useWorkingCopiesStore()
const toast = useErrorToast()

const items = computed(() => store.items)
const collapsedRoots = ref<Set<string>>(new Set())

const groups = computed(() => {
  const map = new Map<string, { root: string; copies: typeof store.items }>()
  for (const wc of store.items) {
    const root = wc.repositoryRoot ?? wc.url ?? '未知远端'
    if (!map.has(root)) map.set(root, { root, copies: [] })
    map.get(root)!.copies.push(wc)
  }
  return [...map.values()].sort((a, b) => a.root.localeCompare(b.root))
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

function toggleRoot(root: string) {
  const next = new Set(collapsedRoots.value)
  if (next.has(root)) next.delete(root)
  else next.add(root)
  collapsedRoots.value = next
}
</script>

<template>
  <div class="wc-list">
    <div class="wc-toolbar">
      <Button size="sm" @click="pickAndAdd">添加工作副本</Button>
    </div>
    <div class="wc-scroll">
      <div v-if="items.length === 0" class="wc-empty">
        <EmptyState description="还没有工作副本，点上面按钮选一个本地目录" />
      </div>
      <div v-for="group in groups" :key="group.root" class="wc-group">
        <button class="root-row" type="button" @click="toggleRoot(group.root)">
          <span :class="['chevron', { collapsed: collapsedRoots.has(group.root) }]" />
          <span class="root-icon" />
          <span class="root-name" :title="group.root">{{ rootLabel(group.root) }}</span>
          <span class="root-count mono">{{ group.copies.length }}</span>
        </button>
        <div v-if="!collapsedRoots.has(group.root)" class="copy-list">
          <div
            v-for="wc in group.copies"
            :key="wc.id"
            :class="['wc-item', { active: wc.id === store.selectedId }]"
            @click="store.select(wc.id)"
          >
            <div class="wc-name" :title="wc.path">
              <span class="copy-icon" />
              {{ shortPath(wc.path) }}
            </div>
            <div class="wc-meta mono">
              <span v-if="wc.revision">r{{ wc.revision }}</span>
              <span v-if="wc.url" class="wc-url" :title="wc.url">{{ wc.url }}</span>
            </div>
            <div class="wc-actions" @click.stop>
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button size="xs" variant="ghost" @click="refresh(wc.id)">刷新</Button>
                  </TooltipTrigger>
                  <TooltipContent>重新读取 svn info</TooltipContent>
                </Tooltip>
              </TooltipProvider>
              <Button size="xs" variant="ghost" class="danger-action" @click="remove(wc.id)">
                移除
              </Button>
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
  height: 100%;
  background: var(--sidebar-bg);
}
.wc-toolbar {
  min-height: 38px;
  padding: 7px 10px;
  border-bottom: 1px solid var(--border);
  background: var(--toolbar-bg);
}
.wc-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow: auto;
  padding: 6px 0;
}
.wc-empty {
  padding: 24px 12px;
  text-align: center;
}
.wc-item {
  margin: 3px 6px 3px 22px;
  padding: 8px;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  cursor: pointer;
}
.wc-item.active {
  border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
  background: var(--accent-row);
}
.wc-item:hover {
  border-color: var(--border-subtle);
  background: var(--panel-bg-subtle);
}
.wc-name {
  color: var(--text-strong);
  font-weight: 500;
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-meta {
  font-size: 11px;
  color: var(--text-muted);
  display: flex;
  gap: 6px;
  margin-top: 2px;
}
.wc-url {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-actions {
  display: flex;
  gap: 6px;
  margin-top: 6px;
}
.danger-action {
  color: var(--destructive);
}
.wc-group {
  margin-bottom: 6px;
}
.root-row {
  width: calc(100% - 12px);
  margin: 0 6px 2px;
  min-height: 28px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text);
  display: grid;
  grid-template-columns: 12px 16px minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
  cursor: pointer;
  text-align: left;
  font: inherit;
  padding: 4px 6px;
}
.root-row:hover {
  background: var(--panel-bg-subtle);
}
.chevron {
  width: 0;
  height: 0;
  border-left: 4px solid transparent;
  border-right: 4px solid transparent;
  border-top: 6px solid var(--text-muted);
}
.chevron.collapsed {
  transform: rotate(-90deg);
}
.root-icon,
.copy-icon {
  display: inline-block;
  flex: none;
}
.root-icon {
  width: 13px;
  height: 13px;
  border-radius: 50%;
  border: 1px solid color-mix(in srgb, var(--accent) 72%, var(--border));
  background: var(--accent-soft);
}
.copy-icon {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  background: var(--file-soft);
  border: 1px solid color-mix(in srgb, var(--file) 48%, var(--border));
}
.root-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
  color: var(--text-strong);
}
.root-count {
  color: var(--text-muted);
  font-size: 11px;
}
</style>
