<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  Cloud,
  Pencil,
  Plug,
  Plus,
  Trash2,
} from 'lucide-vue-next'

import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAppToast } from '@/composables/use-app-toast'
import { confirm } from '@/composables/use-confirm-dialog'
import { api, describeError } from '../api/svn'
import { useRepositoriesStore } from '../stores/repositories'
import type { RepositoryEntry } from '../types/svn'

const store = useRepositoriesStore()
const toast = useAppToast()

const emit = defineEmits<{ select: [id: string] }>()

function selectRepo(id: string) {
  store.select(id)
  emit('select', id)
}

const showModal = ref(false)
const editingId = ref<string | undefined>()
const name = ref('')
const url = ref('')
const username = ref('')
const testing = ref(false)

onMounted(() => {
  store.reload().catch((e) => toast.error('加载仓库失败', describeError(e)))
})

function openCreate() {
  editingId.value = undefined
  name.value = ''
  url.value = ''
  username.value = ''
  showModal.value = true
}

function openEdit(repo: RepositoryEntry) {
  editingId.value = repo.id
  name.value = repo.name
  url.value = repo.url
  username.value = repo.username ?? ''
  showModal.value = true
}

async function save() {
  try {
    await store.save({
      id: editingId.value,
      name: name.value.trim(),
      url: url.value.trim(),
      username: username.value.trim() || undefined,
    })
    showModal.value = false
  } catch (e) {
    toast.error('保存失败', describeError(e))
  }
}

async function test(repo?: RepositoryEntry) {
  const target = repo
    ? { id: repo.id, url: repo.url, username: repo.username ?? undefined }
    : {
        id: editingId.value,
        url: url.value.trim(),
        username: username.value.trim() || undefined,
      }
  testing.value = true
  try {
    const info = await api.testRepositoryConnection(target)
    toast.success(`连接成功：r${info.revision}`)
    await store.reload()
  } catch (e) {
    toast.error('连接失败', describeError(e))
  } finally {
    testing.value = false
  }
}

async function remove(id: string) {
  const ok = await confirm({
    title: '删除仓库配置',
    content: '删除这个仓库配置？本地工作副本不会受到影响。',
    confirmText: '删除',
    destructive: true,
  })
  if (!ok) return
  try {
    await store.remove(id)
  } catch (e) {
    toast.error('删除失败', describeError(e))
  }
}
</script>

<template>
  <section class="repo-section">
    <div class="section-head">
      <span class="section-title">远端仓库</span>
      <Button size="xs" variant="ghost" class="head-action" @click="openCreate">
        <Plus class="icon-xs" />
        添加
      </Button>
    </div>
    <div class="repo-scroll">
      <div v-if="store.items.length === 0" class="empty-text">
        暂无远端仓库
      </div>
      <div
        v-for="repo in store.items"
        :key="repo.id"
        :class="['repo-item', { active: repo.id === store.selectedId }]"
        @click="selectRepo(repo.id)"
      >
        <Cloud class="repo-icon" />
        <div class="repo-text">
          <div class="repo-name">{{ repo.name }}</div>
          <div class="repo-url mono" :title="repo.url">{{ repo.url }}</div>
          <div v-if="repo.username || repo.lastAccessedAt" class="repo-meta">
            <Badge v-if="repo.username" class="user-pill">{{ repo.username }}</Badge>
            <span v-if="repo.lastAccessedAt" class="meta-time">
              {{ new Date(repo.lastAccessedAt).toLocaleDateString() }}
            </span>
          </div>
        </div>
        <div class="repo-actions" @click.stop>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  class="row-icon-btn"
                  :disabled="testing"
                  @click="test(repo)"
                >
                  <Plug class="icon-xs" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>测试连接</TooltipContent>
            </Tooltip>
          </TooltipProvider>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  class="row-icon-btn"
                  @click="openEdit(repo)"
                >
                  <Pencil class="icon-xs" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>编辑</TooltipContent>
            </Tooltip>
          </TooltipProvider>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  class="row-icon-btn danger-action"
                  @click="remove(repo.id)"
                >
                  <Trash2 class="icon-xs" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>删除</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>
    </div>

    <Dialog v-model:open="showModal">
      <DialogContent class="repo-modal">
        <DialogHeader>
          <DialogTitle>远端仓库</DialogTitle>
        </DialogHeader>
        <div class="repo-form">
          <div class="form-row">
            <Label for="repo-name">名称</Label>
            <Input id="repo-name" v-model="name" placeholder="项目名或仓库别名" />
          </div>
          <div class="form-row">
            <Label for="repo-url">URL</Label>
            <Input id="repo-url" v-model="url" placeholder="https://example.com/svn/repo/trunk" />
          </div>
          <div class="form-row">
            <Label for="repo-username">用户名</Label>
            <Input id="repo-username" v-model="username" placeholder="可选" />
          </div>
        </div>
        <DialogFooter>
        <div class="modal-actions">
          <Button variant="outline" :disabled="testing" @click="test()">连接测试</Button>
          <Button @click="save">保存</Button>
        </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
</template>

<style scoped>
.repo-section {
  display: flex;
  flex-direction: column;
  flex: 0 0 42%;
  min-height: 180px;
  max-height: 46%;
  overflow: hidden;
  border-bottom: var(--hairline) solid var(--stroke-soft);
  background: transparent;
}
.section-head {
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
.repo-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 2px 0 8px;
}
.empty-text {
  padding: 8px 14px;
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}

/* ===== repo 行：与 wc-item 同款 source-list 胶囊 ===== */
.repo-item {
  margin: 1px 6px;
  padding: 6px 8px;
  border-radius: var(--radius-row);
  background: transparent;
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: start;
  gap: 8px;
  min-height: 36px;
  cursor: pointer;
  transition: background-color 120ms ease-out;
}
.repo-item:hover {
  background: color-mix(in srgb, var(--fg) 6%, transparent);
}
.repo-item.active {
  background: var(--accent);
  color: var(--fg-on-accent);
}
.repo-item.active .repo-name {
  color: #fff;
  font-weight: 600;
}
.repo-item.active .repo-url,
.repo-item.active .meta-time {
  color: rgba(255, 255, 255, 0.75);
}
.repo-item.active .repo-actions {
  opacity: 1;
}
.repo-item.active .row-icon-btn {
  color: rgba(255, 255, 255, 0.78);
}
.repo-item.active .row-icon-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.16);
}
.repo-icon {
  width: 14px;
  height: 14px;
  margin-top: 2px;
  color: var(--accent);
  flex: none;
}
.repo-text {
  min-width: 0;
}
.repo-name {
  color: var(--fg-strong);
  font-weight: 500;
  font-size: var(--fs-callout);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.repo-url {
  margin-top: 1px;
  font-size: var(--fs-caption);
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.repo-meta {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-top: 3px;
  font-size: var(--fs-caption);
  color: var(--fg-subtle);
}
.user-pill {
  height: 16px;
  padding: 0 6px;
  font-size: 10px;
  font-weight: 500;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--fg) 8%, transparent);
  color: var(--fg-muted);
  border: 0;
}
.meta-time {
  font-feature-settings: 'tnum';
}
.repo-actions {
  display: flex;
  gap: 2px;
  align-items: center;
  opacity: 0;
  transition: opacity 140ms ease-out;
}
.repo-item:hover .repo-actions {
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
.danger-action:hover {
  color: var(--danger) !important;
}
.icon-xs {
  width: 12px;
  height: 12px;
}

.repo-modal {
  width: min(560px, calc(100vw - 32px));
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel-bg) 94%, transparent);
  backdrop-filter: blur(26px) saturate(150%);
}
.repo-form {
  display: grid;
  gap: 12px;
}
.form-row {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 10px;
  align-items: center;
}
.form-row label {
  color: var(--text-muted);
  font-size: 12px;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.danger-action {
  color: var(--destructive);
}
</style>
