<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { NButton, NEmpty, NScrollbar, NPopconfirm, NTooltip } from 'naive-ui'
import { computed, onMounted } from 'vue'

import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useErrorToast } from '../composables/use-error-toast'

const store = useWorkingCopiesStore()
const toast = useErrorToast()

const items = computed(() => store.items)

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
</script>

<template>
  <div class="wc-list">
    <div class="wc-toolbar">
      <n-button size="small" type="primary" @click="pickAndAdd">添加工作副本</n-button>
    </div>
    <n-scrollbar class="wc-scroll">
      <div v-if="items.length === 0" class="wc-empty">
        <n-empty description="还没有工作副本，点上面按钮选一个本地目录" size="small" />
      </div>
      <div
        v-for="wc in items"
        :key="wc.id"
        :class="['wc-item', { active: wc.id === store.selectedId }]"
        @click="store.select(wc.id)"
      >
        <div class="wc-name" :title="wc.path">
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
    </n-scrollbar>
  </div>
</template>

<style scoped>
.wc-list {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.wc-toolbar {
  padding: 8px;
  border-bottom: 1px solid var(--n-border-color, #ddd);
}
.wc-scroll {
  flex: 1;
  min-height: 0;
}
.wc-empty {
  padding: 24px 12px;
  text-align: center;
}
.wc-item {
  padding: 8px 10px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.15);
  cursor: pointer;
}
.wc-item.active {
  background: rgba(26, 107, 255, 0.1);
}
.wc-item:hover {
  background: rgba(127, 127, 127, 0.08);
}
.wc-name {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-meta {
  font-size: 12px;
  opacity: 0.75;
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
</style>
