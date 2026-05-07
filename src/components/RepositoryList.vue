<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  NButton,
  NForm,
  NFormItem,
  NInput,
  NModal,
  NPopconfirm,
  NTag,
  useMessage,
} from 'naive-ui'

import { api, describeError } from '../api/svn'
import { useRepositoriesStore } from '../stores/repositories'
import type { RepositoryEntry } from '../types/svn'

const emit = defineEmits<{
  checkout: [repo: RepositoryEntry]
  browse: [repo: RepositoryEntry]
}>()

const store = useRepositoriesStore()
const message = useMessage()

const showModal = ref(false)
const editingId = ref<string | undefined>()
const name = ref('')
const url = ref('')
const username = ref('')
const testing = ref(false)

onMounted(() => {
  store.reload().catch((e) => message.error(describeError(e)))
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
    message.error(describeError(e))
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
    message.success(`连接成功：r${info.revision}`)
    await store.reload()
  } catch (e) {
    message.error(describeError(e))
  } finally {
    testing.value = false
  }
}

async function remove(id: string) {
  try {
    await store.remove(id)
  } catch (e) {
    message.error(describeError(e))
  }
}
</script>

<template>
  <section class="repo-section">
    <div class="section-head">
      <span>远端仓库</span>
      <n-button size="tiny" type="primary" @click="openCreate">添加</n-button>
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
            <n-tag v-if="repo.username" size="tiny">{{ repo.username }}</n-tag>
            <span v-if="repo.lastAccessedAt">最近连接 {{ new Date(repo.lastAccessedAt).toLocaleDateString() }}</span>
          </div>
        </div>
        <div class="repo-actions" @click.stop>
          <n-button size="tiny" tertiary :loading="testing" @click="test(repo)">测试</n-button>
          <n-button size="tiny" tertiary type="primary" @click="emit('browse', repo)">浏览</n-button>
          <n-button size="tiny" tertiary @click="emit('checkout', repo)">检出</n-button>
          <n-button size="tiny" tertiary @click="openEdit(repo)">编辑</n-button>
          <n-popconfirm @positive-click="remove(repo.id)">
            <template #trigger>
              <n-button size="tiny" tertiary type="error">删除</n-button>
            </template>
            删除这个仓库配置？
          </n-popconfirm>
        </div>
      </div>
    </div>

    <n-modal v-model:show="showModal" preset="card" title="远端仓库" class="repo-modal">
      <n-form label-placement="left" label-width="80" size="small">
        <n-form-item label="名称" required>
          <n-input v-model:value="name" placeholder="项目名或仓库别名" />
        </n-form-item>
        <n-form-item label="URL" required>
          <n-input v-model:value="url" placeholder="https://example.com/svn/repo/trunk" />
        </n-form-item>
        <n-form-item label="用户名">
          <n-input v-model:value="username" placeholder="可选" />
        </n-form-item>
      </n-form>
      <template #footer>
        <div class="modal-actions">
          <n-button :loading="testing" @click="test()">连接测试</n-button>
          <n-button type="primary" @click="save">保存</n-button>
        </div>
      </template>
    </n-modal>
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
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--accent-soft) 34%, transparent), transparent 180px),
    var(--sidebar-bg);
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 10px 8px;
  font-size: 13px;
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
  margin: 0 8px 8px;
  padding: 9px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--panel-bg) 80%, transparent), transparent),
    var(--panel-bg);
  box-shadow: var(--shadow-sm);
  cursor: pointer;
}
.repo-item:hover {
  border-color: color-mix(in srgb, var(--accent) 34%, var(--border));
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--accent-soft) 70%, transparent), transparent 260px),
    var(--panel-bg-subtle);
}
.repo-main {
  cursor: pointer;
}
.repo-name {
  color: var(--text-strong);
  font-weight: 500;
  font-size: 13px;
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
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
