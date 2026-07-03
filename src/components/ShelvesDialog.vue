<script setup lang="ts">
import { ref, watch } from 'vue'
import { Archive, ChevronDown, ChevronRight, RotateCcw, Trash2 } from 'lucide-vue-next'

import { api } from '../api/svn'
import type { ShelfEntry, WorkingCopyEntry } from '../types/svn'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import LoadingSpinner from '@/components/ui-local/LoadingSpinner.vue'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { useConfirmDialog } from '../composables/use-confirm-dialog'
import { useErrorToast } from '../composables/use-error-toast'
import { useAppToast } from '../composables/use-app-toast'

const props = defineProps<{
  workingCopy: WorkingCopyEntry
  open: boolean
  // 状态页已勾选、可搁置的文件；非空时可选择只搁置这些文件
  checkedPaths?: string[]
}>()
const emit = defineEmits<{ 'update:open': [boolean]; changed: [] }>()

const { confirm } = useConfirmDialog()
const toast = useErrorToast()
const appToast = useAppToast()

const shelves = ref<ShelfEntry[]>([])
const loading = ref(false)
const working = ref(false)
const name = ref('')
const expanded = ref<Set<string>>(new Set())
// 搁置范围：勾选了文件时默认只搁置勾选项
const scope = ref<'selected' | 'all'>('all')

async function reload() {
  loading.value = true
  try {
    shelves.value = await api.shelfList(props.workingCopy.path)
  } catch (e) {
    toast(e, '读取搁置列表失败')
  } finally {
    loading.value = false
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      name.value = ''
      expanded.value = new Set()
      scope.value = (props.checkedPaths?.length ?? 0) > 0 ? 'selected' : 'all'
      void reload()
    }
  },
)

async function createShelf() {
  if (working.value) return
  const selected = scope.value === 'selected' ? (props.checkedPaths ?? []) : []
  const scopeText =
    selected.length > 0
      ? `将只搁置勾选的 ${selected.length} 个文件：\n${selected.join('\n')}\n\n`
      : '将把工作副本当前的全部版本化修改保存为补丁。\n\n'
  const ok = await confirm({
    title: '搁置修改',
    content:
      scopeText +
      '搁置后对应文件会还原到未修改状态（类似 git stash）。' +
      '未跟踪文件不受影响，二进制修改会被拒绝。\n\n' +
      `目标：${props.workingCopy.path}`,
    confirmText: '确认搁置',
    destructive: true,
  })
  if (!ok) return
  working.value = true
  try {
    const entry = await api.shelfCreate(
      props.workingCopy.path,
      name.value,
      selected.length > 0 ? selected : undefined,
    )
    appToast.success('已搁置', `${entry.name}（${entry.fileCount} 个文件）`)
    name.value = ''
    await reload()
    emit('changed')
  } catch (e) {
    toast(e, '搁置失败')
  } finally {
    working.value = false
  }
}

async function applyShelf(shelf: ShelfEntry) {
  if (working.value) return
  const ok = await confirm({
    title: '恢复搁置',
    content:
      `将把搁置补丁应用回工作副本：\n${shelf.wcPath}\n\n` +
      '应用后搁置记录会保留，确认无误后可手动删除，避免重复应用。',
    confirmText: '恢复',
  })
  if (!ok) return
  working.value = true
  try {
    await api.shelfApply(shelf.id)
    appToast.success('已恢复搁置', shelf.name)
    emit('changed')
  } catch (e) {
    toast(e, '恢复搁置失败')
  } finally {
    working.value = false
  }
}

async function deleteShelf(shelf: ShelfEntry) {
  if (working.value) return
  const ok = await confirm({
    title: '删除搁置',
    content: `搁置补丁将被永久删除，未恢复的修改无法找回：\n${shelf.name}`,
    confirmText: '确认删除',
    destructive: true,
  })
  if (!ok) return
  working.value = true
  try {
    await api.shelfDelete(shelf.id)
    await reload()
  } catch (e) {
    toast(e, '删除搁置失败')
  } finally {
    working.value = false
  }
}

function toggleExpand(id: string) {
  const next = new Set(expanded.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expanded.value = next
}

function formatTime(iso: string) {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString('zh-CN', { hour12: false })
}
</script>

<template>
  <Dialog :open="open" @update:open="(v) => emit('update:open', v)">
    <DialogContent class="shelves-modal">
      <DialogHeader>
        <DialogTitle>搁置改动</DialogTitle>
      </DialogHeader>

      <div class="shelf-create">
        <Input
          v-model="name"
          placeholder="搁置名称（可选）"
          :disabled="working"
          @keyup.enter="createShelf"
        />
        <Button size="sm" :disabled="working" @click="createShelf">
          <Archive class="icon-xs" />
          {{ working ? '处理中…' : '搁置修改' }}
        </Button>
      </div>
      <div v-if="(checkedPaths?.length ?? 0) > 0" class="scope-switch">
        <span class="scope-label">搁置范围</span>
        <Button
          size="xs"
          :variant="scope === 'selected' ? 'secondary' : 'ghost'"
          :disabled="working"
          @click="scope = 'selected'"
        >
          已勾选 {{ checkedPaths!.length }} 个文件
        </Button>
        <Button
          size="xs"
          :variant="scope === 'all' ? 'secondary' : 'ghost'"
          :disabled="working"
          @click="scope = 'all'"
        >
          全部修改
        </Button>
      </div>

      <div class="shelf-list">
        <LoadingSpinner v-if="loading" />
        <EmptyState v-else-if="shelves.length === 0" description="该工作副本没有搁置记录" />
        <div v-for="shelf in shelves" v-else :key="shelf.id" class="shelf-item">
          <div class="shelf-row" @click="toggleExpand(shelf.id)">
            <component
              :is="expanded.has(shelf.id) ? ChevronDown : ChevronRight"
              class="icon-xs chevron"
            />
            <div class="shelf-info">
              <span class="shelf-name">{{ shelf.name }}</span>
              <span class="shelf-meta">
                {{ formatTime(shelf.createdAt) }} · {{ shelf.fileCount }} 个文件
              </span>
            </div>
            <div class="shelf-actions" @click.stop>
              <Button size="xs" variant="ghost" :disabled="working" @click="applyShelf(shelf)">
                <RotateCcw class="icon-xs" />
                恢复
              </Button>
              <Button
                size="xs"
                variant="ghost"
                class="danger-action"
                :disabled="working"
                @click="deleteShelf(shelf)"
              >
                <Trash2 class="icon-xs" />
              </Button>
            </div>
          </div>
          <pre v-if="expanded.has(shelf.id)" class="shelf-status mono">{{ shelf.statusText }}</pre>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.shelves-modal {
  width: 560px;
  max-width: 92vw;
}

.shelf-create {
  display: flex;
  gap: 8px;
  align-items: center;
}

.shelf-create :deep(input) {
  flex: 1;
}

.scope-switch {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
}

.scope-label {
  font-size: 11px;
  color: var(--muted-foreground);
}

.shelf-list {
  margin-top: 12px;
  max-height: 46vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.shelf-item {
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}

.shelf-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  cursor: pointer;
}

.shelf-row:hover {
  background: color-mix(in srgb, var(--muted) 55%, transparent);
}

.chevron {
  flex: none;
  color: var(--muted-foreground);
}

.shelf-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shelf-name {
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shelf-meta {
  font-size: 11px;
  color: var(--muted-foreground);
}

.shelf-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.shelf-status {
  margin: 0;
  padding: 8px 10px;
  border-top: 1px solid var(--border);
  font-size: 11px;
  line-height: 1.5;
  max-height: 180px;
  overflow: auto;
  background: color-mix(in srgb, var(--muted) 40%, transparent);
  white-space: pre-wrap;
  word-break: break-all;
}

.icon-xs {
  width: 13px;
  height: 13px;
}

.danger-action {
  color: var(--destructive);
}
</style>
