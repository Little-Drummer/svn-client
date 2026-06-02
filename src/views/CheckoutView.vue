<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAppToast } from '@/composables/use-app-toast'
import TaskOutput from '../components/TaskOutput.vue'
import { api, describeError } from '../api/svn'
import { useTasksStore } from '../stores/tasks'
import { useWorkingCopiesStore } from '../stores/workingCopies'
import type { RepositoryEntry } from '../types/svn'

const props = defineProps<{ repository?: RepositoryEntry | null }>()

const tasksStore = useTasksStore()
const wcStore = useWorkingCopiesStore()
const toast = useAppToast()

const url = ref('')
const targetPath = ref('')
const revision = ref('')
const username = ref('')
const password = ref('')

const taskId = ref<string | null>(null)
const running = computed(() => {
  if (!taskId.value) return false
  const t = tasksStore.tasks.get(taskId.value)
  return !!t && !t.finished
})

async function pickTarget() {
  const dir = await open({ directory: true, multiple: false, title: '选择检出目录' })
  if (typeof dir === 'string') {
    targetPath.value = dir
  }
}

async function start() {
  if (!url.value.trim() || !targetPath.value.trim()) {
    toast.warning('URL 和目标目录都不能为空')
    return
  }
  try {
    taskId.value = await launchCheckout({
      url: url.value.trim(),
      targetPath: targetPath.value.trim(),
      revision: revision.value || undefined,
      username: username.value || undefined,
      password: password.value || undefined,
    })
  } catch (e) {
    toast.error('启动检出失败', describeError(e))
  }
}

async function launchCheckout(params: {
  url: string
  targetPath: string
  revision?: string
  username?: string
  password?: string
}): Promise<string> {
  const id = await api.startCheckout(params)
  // 等价命令行：密码打码，避免泄露到界面
  const parts = ['svn', 'checkout']
  if (params.revision) parts.push('-r', params.revision)
  if (params.username) parts.push('--username', params.username)
  if (params.password) parts.push('--password', '••••••')
  parts.push(params.url, params.targetPath)
  tasksStore.register({
    taskId: id,
    kind: 'checkout',
    title: `检出 ${params.url} → ${params.targetPath}`,
    command: parts.join(' '),
    retry: () => launchCheckout(params),
  })
  return id
}

watch(
  () => props.repository?.id,
  () => {
    if (!props.repository) return
    url.value = props.repository.url
    username.value = props.repository.username ?? ''
  },
  { immediate: true },
)

watch(
  () => taskId.value && tasksStore.tasks.get(taskId.value)?.finished,
  async (finished) => {
    if (!finished) return
    const t = taskId.value ? tasksStore.tasks.get(taskId.value) : null
    if (t?.success) {
      // 自动加入工作副本列表
      try {
        await wcStore.add(targetPath.value)
        toast.success('检出完成，已加入工作副本列表')
      } catch (e) {
        toast.warning('检出完成但加入工作副本列表失败', describeError(e))
      }
    }
  },
)
</script>

<template>
  <div class="checkout-view">
    <section class="card">
      <header class="card-head">
        <h2 class="card-title">检出 (svn checkout)</h2>
        <span class="card-hint">从远端 URL 拉取一份本地副本</span>
      </header>
      <div class="checkout-form">
        <div class="form-row">
          <Label for="checkout-url" class="form-label">远端 URL</Label>
          <Input
            id="checkout-url"
            v-model="url"
            placeholder="https://example.com/svn/repo/trunk"
            :disabled="running"
          />
        </div>
        <div class="form-row">
          <Label for="checkout-target" class="form-label">本地目录</Label>
          <div class="input-group">
            <Input
              id="checkout-target"
              v-model="targetPath"
              placeholder="/path/to/local/folder"
              :disabled="running"
            />
            <Button variant="outline" :disabled="running" @click="pickTarget">选择…</Button>
          </div>
        </div>
        <div class="form-row">
          <Label for="checkout-revision" class="form-label">Revision</Label>
          <Input id="checkout-revision" v-model="revision" placeholder="留空 = HEAD" :disabled="running" />
        </div>
        <div class="form-row">
          <Label for="checkout-username" class="form-label">用户名</Label>
          <Input id="checkout-username" v-model="username" placeholder="可选" :disabled="running" />
        </div>
        <div class="form-row">
          <Label for="checkout-password" class="form-label">密码</Label>
          <Input
            id="checkout-password"
            v-model="password"
            type="password"
            placeholder="可选"
            :disabled="running"
          />
        </div>
        <div class="form-actions">
          <Button :disabled="running" @click="start">{{ running ? '检出中' : '开始检出' }}</Button>
        </div>
      </div>
    </section>

    <div class="output-wrap">
      <TaskOutput :task-id="taskId" @retried="taskId = $event" />
    </div>
  </div>
</template>

<style scoped>
.checkout-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 16px;
  gap: 14px;
  background: var(--mat-window);
  overflow: auto;
}
.card {
  flex-shrink: 0;
  border-radius: 10px;
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 2px rgba(0, 0, 0, 0.04);
  padding: 16px 18px 14px;
}
.card-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: var(--hairline) solid var(--stroke-soft);
}
.card-title {
  margin: 0;
  font-size: var(--fs-headline);
  font-weight: 600;
  color: var(--fg-strong);
  letter-spacing: -0.01em;
}
.card-hint {
  font-size: var(--fs-callout);
  color: var(--fg-muted);
}
.checkout-form {
  display: grid;
  gap: 10px;
}
.form-row {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}
.form-label {
  color: var(--fg-muted);
  font-size: var(--fs-callout);
  font-weight: 500;
  text-align: right;
}
.input-group {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 4px;
}
.output-wrap {
  flex: 1;
  min-height: 220px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 10px;
  background: var(--mat-elevated);
  box-shadow:
    inset 0 0 0 0.5px var(--stroke),
    0 1px 2px rgba(0, 0, 0, 0.04);
}
</style>
