<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { NButton, NEmpty, NPopconfirm, NTooltip } from 'naive-ui'
import { computed, onMounted, ref } from 'vue'

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
      <n-button size="small" type="primary" @click="pickAndAdd">添加工作副本</n-button>
    </div>
    <div class="wc-scroll">
      <div v-if="items.length === 0" class="wc-empty">
        <n-empty description="还没有工作副本，点上面按钮选一个本地目录" size="small" />
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
              <n-tooltip>
                <template #trigger>
                  <n-button size="tiny" tertiary @click="refresh(wc.id)">刷新</n-button>
                </template>
                重新读取 svn info
              </n-tooltip>
              <n-popconfirm @positive-click="remove(wc.id)">
                <template #trigger>
                  <n-button size="tiny" tertiary type="error">移除</n-button>
                </template>
                移除这个工作副本？只是从列表里去掉，不会删除磁盘文件。
              </n-popconfirm>
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
  padding: 10px;
  border-bottom: 1px solid var(--border);
}
.wc-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow: auto;
  padding: 8px 0;
}
.wc-empty {
  padding: 24px 12px;
  text-align: center;
}
.wc-item {
  margin: 6px 8px 8px 24px;
  padding: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--panel-bg) 80%, transparent), transparent),
    var(--panel-bg);
  box-shadow: var(--shadow-sm);
  cursor: pointer;
}
.wc-item.active {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--border));
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 82%, transparent), transparent 260px),
    var(--panel-bg);
}
.wc-item:hover {
  border-color: color-mix(in srgb, var(--accent) 26%, var(--border));
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 58%, transparent), transparent 260px),
    var(--panel-bg-subtle);
}
.wc-name {
  color: var(--text-strong);
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-meta {
  font-size: 12px;
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
.wc-group {
  margin-bottom: 6px;
}
.root-row {
  width: calc(100% - 16px);
  margin: 0 8px 4px;
  min-height: 30px;
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
  background: var(--accent-soft);
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
  width: 15px;
  height: 15px;
  border-radius: 50%;
  border: 2px solid var(--accent);
  box-shadow: inset 0 0 0 3px var(--accent-soft);
}
.copy-icon {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  background: linear-gradient(180deg, var(--file-soft), color-mix(in srgb, var(--file) 28%, var(--file-soft)));
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
