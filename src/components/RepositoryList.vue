<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { Badge } from '@/components/ui/badge'
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

const emit = defineEmits<{
  checkout: [repo: RepositoryEntry]
  browse: [repo: RepositoryEntry]
}>()

const store = useRepositoriesStore()
const toast = useAppToast()

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
      <span>远端仓库</span>
      <Button size="xs" @click="openCreate">添加</Button>
    </div>
    <div class="repo-scroll">
      <div v-if="store.items.length === 0" class="empty-text">暂无远端仓库</div>
      <div
        v-for="repo in store.items"
        :key="repo.id"
        class="repo-item"
        @click="emit('browse', repo)"
      >
        <div class="repo-main">
          <div class="repo-name">{{ repo.name }}</div>
          <div class="repo-url mono" :title="repo.url">{{ repo.url }}</div>
          <div class="repo-meta">
            <Badge v-if="repo.username" variant="outline">{{ repo.username }}</Badge>
            <span v-if="repo.lastAccessedAt">最近连接 {{ new Date(repo.lastAccessedAt).toLocaleDateString() }}</span>
          </div>
        </div>
        <div class="repo-actions" @click.stop>
          <Button size="xs" variant="ghost" :disabled="testing" @click="test(repo)">测试</Button>
          <Button size="xs" variant="ghost" @click="emit('browse', repo)">浏览</Button>
          <Button size="xs" variant="ghost" @click="emit('checkout', repo)">检出</Button>
          <Button size="xs" variant="ghost" @click="openEdit(repo)">编辑</Button>
          <Button size="xs" variant="ghost" class="danger-action" @click="remove(repo.id)">删除</Button>
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
  flex: 0 0 40%;
  min-height: 180px;
  max-height: 42%;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
  background: var(--sidebar-bg);
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
  padding: 8px 10px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-strong);
}
.repo-scroll {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-top: 2px;
}
.empty-text {
  padding: 8px 10px;
  font-size: 12px;
  color: var(--text-muted);
}
.repo-item {
  margin: 0 6px 4px;
  padding: 8px;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  cursor: pointer;
}
.repo-item:hover {
  border-color: var(--border-subtle);
  background: var(--panel-bg-subtle);
}
.repo-main {
  cursor: pointer;
}
.repo-name {
  color: var(--text-strong);
  font-weight: 500;
  font-size: 12px;
}
.repo-url {
  margin-top: 2px;
  font-size: 12px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.repo-meta {
  display: flex;
  gap: 6px;
  align-items: center;
  min-height: 18px;
  margin-top: 4px;
  font-size: 11px;
  color: var(--text-muted);
}
.repo-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 8px;
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
