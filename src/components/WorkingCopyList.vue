<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onMounted, ref } from 'vue'
import {
  ChevronDown,
  FolderGit2,
  HardDrive,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
} from 'lucide-vue-next'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import EmptyState from '@/components/ui-local/EmptyState.vue'
import { confirm } from '@/composables/use-confirm-dialog'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import { useRepositoriesStore } from '../stores/repositories'
import { useErrorToast } from '../composables/use-error-toast'
import type { WorkingCopyEntry } from '../types/svn'
import {
  getSmartLabel,
  getSmartSubtitle,
  getGroupKey,
  getFullTitle as fullTitle,
  getDecodedBranchInfo,
  getDecodedUrl,
} from '../lib/utils'

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
  const map = new Map<string, { key: string; copies: typeof store.items }>()
  for (const wc of store.items) {
    const key = getGroupKey(wc)
    if (!map.has(key)) map.set(key, { key, copies: [] })
    map.get(key)!.copies.push(wc)
  }
  return [...map.values()]
    .map((g) => {
      // 为本地项目组生成友好标题；仓库组回退旧逻辑
      let label = g.key
      if (g.key.startsWith('local:')) {
        label = g.key.slice(6)
      } else if (g.key.startsWith('repo:')) {
        const root = g.key.slice(5)
        label = repoNameForRoot(root) ?? rootLabel(root)
      }
      return { key: g.key, copies: g.copies, label }
    })
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

function toggleRoot(key: string) {
  const next = new Set(collapsedRoots.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  collapsedRoots.value = next
}

// 内联编辑状态：彻底避开 prompt + tooltip + hover 的事件竞争问题
const editingId = ref<string | null>(null)
const editingValue = ref('')
const editingInputRef = ref<any>(null)

async function startEdit(wc: WorkingCopyEntry) {
  editingId.value = wc.id
  editingValue.value = wc.displayName || getSmartLabel(wc)
  await nextTick()
  // 聚焦原生 input（Input 组件根元素即 input）
  try {
    const el = editingInputRef.value?.$el || editingInputRef.value
    const input = el?.focus ? el : el?.querySelector?.('input')
    if (input) {
      input.focus()
      if (input.select) input.select()
    }
  } catch {}
}

async function commitEdit() {
  const id = editingId.value
  if (!id) return
  const val = editingValue.value.trim()
  const newName = val ? val : null
  try {
    await store.setDisplayName(id, newName)
  } catch (e) {
    toast(e, '保存显示名称失败')
  }
  editingId.value = null
  editingValue.value = ''
}

function cancelEdit() {
  editingId.value = null
  editingValue.value = ''
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
      <div v-for="group in groups" :key="group.key" class="wc-group">
        <button class="root-row" type="button" @click="toggleRoot(group.key)">
          <ChevronDown
            :class="['root-chevron', { collapsed: collapsedRoots.has(group.key) }]"
          />
          <FolderGit2 class="root-icon" />
          <span
            class="root-name"
            :title="group.key.startsWith('local:') ? '本地项目分组：' + group.label : getDecodedUrl(group.key.replace(/^repo:/, ''))"
          >{{ group.label }}</span>
          <span class="root-count mono">{{ group.copies.length }}</span>
        </button>
        <div v-if="!collapsedRoots.has(group.key)" class="copy-list">
          <div
            v-for="wc in group.copies"
            :key="wc.id"
            :class="['wc-item', { active: props.showActive && wc.id === store.selectedId }]"
            @click="selectWc(wc.id)"
          >
            <div class="wc-row-main">
              <HardDrive class="wc-icon" />
              <div class="wc-text">
                <div class="wc-name-wrap" @click.stop>
                  <template v-if="editingId === wc.id">
                    <Input
                      ref="editingInputRef"
                      v-model="editingValue"
                      class="edit-name-input"
                      @keydown.enter="commitEdit"
                      @keydown.esc="cancelEdit"
                      @blur="commitEdit"
                    />
                  </template>
                  <div
                    v-else
                    class="wc-name"
                    :title="fullTitle(wc)"
                    @dblclick="startEdit(wc)"
                  >
                    {{ getSmartLabel(wc) }}
                  </div>
                </div>
                <div v-if="editingId !== wc.id" class="wc-meta">
                  <span v-if="wc.revision" class="meta-rev mono">r{{ wc.revision }}</span>
                  <span
                    v-if="getSmartSubtitle(wc)"
                    class="wc-url mono"
                    :title="getDecodedBranchInfo(wc) || ''"
                  >{{ getSmartSubtitle(wc) }}</span>
                </div>
              </div>
              <div v-if="editingId !== wc.id" class="wc-actions" @click.stop>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        class="row-icon-btn"
                        @click.stop="refresh(wc.id)"
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
                        class="row-icon-btn"
                        @click.stop="startEdit(wc)"
                      >
                        <Pencil class="icon-xs" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>编辑显示名称（自动推断或自定义）</TooltipContent>
                  </Tooltip>
                </TooltipProvider>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        class="row-icon-btn danger-action"
                        @click.stop="remove(wc.id)"
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
  position: relative;
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr);
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
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  z-index: 10;
  display: flex;
  gap: 2px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 140ms ease-out;
}
.wc-item:hover .wc-actions,
.wc-item.active .wc-actions {
  opacity: 1;
  pointer-events: auto;
}
.wc-item:hover .wc-actions {
  /* subtle mask so long name text doesn't bleed through gaps between action buttons */
  background: color-mix(in srgb, var(--fg) 3%, transparent);
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

/* ===== 内联编辑输入（替换名称行，适配列表密度）===== */
.wc-name-wrap {
  min-width: 0;
}
.edit-name-input {
  height: 24px;
  font-size: var(--fs-callout);
  font-weight: 500;
  /* 让输入框在行内不被绝对定位的 actions 遮挡，actions 编辑时已隐藏 */
  padding-right: 4px;
}
.wc-item.active .edit-name-input {
  color: #fff;
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.3);
}
</style>
